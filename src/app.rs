use crate::{config::Config, error::AppError};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    routing::{get, post},
};
use base64::Engine;
use chrono::{NaiveDate, Utc};
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Config,
    pub http: Client,
}
type Shared = Arc<AppState>;

#[derive(Deserialize)]
struct CustomerInput {
    name: String,
    email: String,
}
#[derive(Serialize)]
struct Customer {
    id: Uuid,
    name: String,
    email: String,
    created_at: chrono::DateTime<Utc>,
}
#[derive(Serialize, Deserialize, Clone)]
struct LineItem {
    description: String,
    quantity: i64,
    unit_amount_cents: i64,
}
#[derive(Deserialize)]
struct InvoiceInput {
    customer_id: Uuid,
    due_date: NaiveDate,
    line_items: Vec<LineItem>,
}
#[derive(Serialize)]
struct Invoice {
    id: Uuid,
    customer_id: Uuid,
    total_cents: i64,
    state: String,
    due_date: NaiveDate,
    line_items: Vec<LineItem>,
}
#[derive(Deserialize)]
struct StateQuery {
    state: Option<String>,
}
#[derive(Deserialize)]
struct PayInput {
    card_token: String,
}
#[derive(Serialize, Deserialize, Clone)]
struct PayResult {
    payment_attempt_id: Uuid,
    status: String,
    failure_code: Option<String>,
    psp_reference: Option<Uuid>,
}
#[derive(Deserialize)]
struct EndpointInput {
    url: String,
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;
    sqlx::migrate!().run(&db).await?;
    seed_dev_key(&db, &config).await?;
    let state = Arc::new(AppState {
        db,
        config: config.clone(),
        http: Client::new(),
    });
    let listener = tokio::net::TcpListener::bind(&config.bind_address).await?;
    let worker_state = state.clone();
    tokio::spawn(async move {
        webhook_worker(worker_state).await;
    });
    axum::serve(listener, router(state)).await?;
    Ok(())
}

pub fn router(state: Shared) -> Router {
    Router::new()
        .route("/customers", post(create_customer).get(list_customers))
        .route("/customers/:id", get(get_customer))
        .route("/invoices", post(create_invoice).get(list_invoices))
        .route("/invoices/:id", get(get_invoice))
        .route("/invoices/:id/pay", post(pay_invoice))
        .route(
            "/webhook-endpoints",
            post(register_endpoint).get(list_endpoints),
        )
        .route(
            "/webhook-endpoints/:id",
            axum::routing::delete(disable_endpoint),
        )
        .with_state(state)
}

async fn auth(headers: &HeaderMap, state: &AppState) -> Result<Uuid, AppError> {
    let value = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::Unauthorized)?;
    let encoded = value.strip_prefix("Basic ").ok_or(AppError::Unauthorized)?;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| AppError::Unauthorized)?;
    let credentials = String::from_utf8(decoded).map_err(|_| AppError::Unauthorized)?;
    let (key_id, secret) = credentials.split_once(':').ok_or(AppError::Unauthorized)?;
    let row = sqlx::query(
        "SELECT business_id, secret_hash FROM api_keys WHERE key_id=$1 AND revoked_at IS NULL",
    )
    .bind(key_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::Unauthorized)?;
    let mut mac = HmacSha256::new_from_slice(state.config.api_key_pepper.as_bytes())
        .map_err(|_| AppError::Unauthorized)?;
    mac.update(secret.as_bytes());
    if hex(&mac.finalize().into_bytes()) != row.get::<String, _>("secret_hash") {
        return Err(AppError::Unauthorized);
    }
    let business_id = row.get("business_id");
    sqlx::query("UPDATE api_keys SET last_used_at=now() WHERE key_id=$1")
        .bind(key_id)
        .execute(&state.db)
        .await?;
    Ok(business_id)
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

async fn create_customer(
    State(s): State<Shared>,
    headers: HeaderMap,
    Json(input): Json<CustomerInput>,
) -> Result<(StatusCode, Json<Customer>), AppError> {
    let business = auth(&headers, &s).await?;
    if input.name.trim().is_empty() || input.email.trim().is_empty() {
        return Err(AppError::BadRequest("name and email are required".into()));
    }
    let id = Uuid::now_v7();
    let created_at = sqlx::query(
        "INSERT INTO customers(id,business_id,name,email) VALUES($1,$2,$3,$4) RETURNING created_at",
    )
    .bind(id)
    .bind(business)
    .bind(&input.name)
    .bind(&input.email)
    .fetch_one(&s.db)
    .await?
    .get("created_at");
    Ok((
        StatusCode::CREATED,
        Json(Customer {
            id,
            name: input.name,
            email: input.email,
            created_at,
        }),
    ))
}
async fn get_customer(
    State(s): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Customer>, AppError> {
    let business = auth(&headers, &s).await?;
    let row = sqlx::query(
        "SELECT id,name,email,created_at FROM customers WHERE id=$1 AND business_id=$2",
    )
    .bind(id)
    .bind(business)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(Customer {
        id: row.get("id"),
        name: row.get("name"),
        email: row.get("email"),
        created_at: row.get("created_at"),
    }))
}
async fn list_customers(
    State(s): State<Shared>,
    headers: HeaderMap,
) -> Result<Json<Vec<Customer>>, AppError> {
    let business = auth(&headers, &s).await?;
    let rows = sqlx::query("SELECT id,name,email,created_at FROM customers WHERE business_id=$1 ORDER BY created_at DESC").bind(business).fetch_all(&s.db).await?;
    Ok(Json(
        rows.into_iter()
            .map(|r| Customer {
                id: r.get("id"),
                name: r.get("name"),
                email: r.get("email"),
                created_at: r.get("created_at"),
            })
            .collect(),
    ))
}

async fn create_invoice(
    State(s): State<Shared>,
    headers: HeaderMap,
    Json(input): Json<InvoiceInput>,
) -> Result<(StatusCode, Json<Invoice>), AppError> {
    let business = auth(&headers, &s).await?;
    if input.line_items.is_empty()
        || input
            .line_items
            .iter()
            .any(|x| x.quantity <= 0 || x.unit_amount_cents < 0 || x.description.trim().is_empty())
    {
        return Err(AppError::BadRequest("invalid line_items".into()));
    }
    let total = input
        .line_items
        .iter()
        .try_fold(0i64, |sum, item| {
            let amount = item
                .quantity
                .checked_mul(item.unit_amount_cents)
                .ok_or(())?;
            sum.checked_add(amount).ok_or(())
        })
        .map_err(|_| AppError::BadRequest("invoice total is too large".into()))?;
    let mut tx = s.db.begin().await?;
    sqlx::query("SELECT id FROM customers WHERE id=$1 AND business_id=$2")
        .bind(input.customer_id)
        .bind(business)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(AppError::NotFound)?;
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO invoices(id,business_id,customer_id,total_cents,state,due_date) VALUES($1,$2,$3,$4,'open',$5)").bind(id).bind(business).bind(input.customer_id).bind(total).bind(input.due_date).execute(&mut *tx).await?;
    for item in &input.line_items {
        sqlx::query("INSERT INTO invoice_line_items(id,invoice_id,description,quantity,unit_amount_cents) VALUES($1,$2,$3,$4,$5)").bind(Uuid::now_v7()).bind(id).bind(&item.description).bind(item.quantity).bind(item.unit_amount_cents).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    enqueue_event(
        &s.db,
        business,
        "invoice.created",
        serde_json::json!({"invoice_id":id,"state":"open"}),
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(Invoice {
            id,
            customer_id: input.customer_id,
            total_cents: total,
            state: "open".into(),
            due_date: input.due_date,
            line_items: input.line_items,
        }),
    ))
}
async fn invoice_for(s: &AppState, business: Uuid, id: Uuid) -> Result<Invoice, AppError> {
    let row = sqlx::query("SELECT customer_id,total_cents,state,due_date FROM invoices WHERE id=$1 AND business_id=$2").bind(id).bind(business).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    let items = sqlx::query("SELECT description,quantity,unit_amount_cents FROM invoice_line_items WHERE invoice_id=$1 ORDER BY created_at").bind(id).fetch_all(&s.db).await?.into_iter().map(|r| LineItem { description:r.get("description"), quantity:r.get("quantity"), unit_amount_cents:r.get("unit_amount_cents") }).collect();
    Ok(Invoice {
        id,
        customer_id: row.get("customer_id"),
        total_cents: row.get("total_cents"),
        state: row.get("state"),
        due_date: row.get("due_date"),
        line_items: items,
    })
}
async fn get_invoice(
    State(s): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Invoice>, AppError> {
    Ok(Json(invoice_for(&s, auth(&headers, &s).await?, id).await?))
}
async fn list_invoices(
    State(s): State<Shared>,
    headers: HeaderMap,
    Query(query): Query<StateQuery>,
) -> Result<Json<Vec<Invoice>>, AppError> {
    let business = auth(&headers, &s).await?;
    let rows = if let Some(state) = query.state {
        sqlx::query(
            "SELECT id FROM invoices WHERE business_id=$1 AND state=$2 ORDER BY created_at DESC",
        )
        .bind(business)
        .bind(state)
        .fetch_all(&s.db)
        .await?
    } else {
        sqlx::query("SELECT id FROM invoices WHERE business_id=$1 ORDER BY created_at DESC")
            .bind(business)
            .fetch_all(&s.db)
            .await?
    };
    let mut result = Vec::new();
    for row in rows {
        result.push(invoice_for(&s, business, row.get("id")).await?);
    }
    Ok(Json(result))
}

async fn pay_invoice(
    State(s): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(input): Json<PayInput>,
) -> Result<Json<PayResult>, AppError> {
    let business = auth(&headers, &s).await?;
    let key = headers
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty())
        .ok_or_else(|| AppError::BadRequest("Idempotency-Key is required".into()))?;
    let fingerprint = hex(&Sha256::digest(input.card_token.as_bytes()));
    // Short-circuit: same business + invoice + key already seen -> replay stored response.
    if let Some(row) = sqlx::query("SELECT request_fingerprint,response FROM idempotency_keys WHERE business_id=$1 AND invoice_id=$2 AND idempotency_key=$3").bind(business).bind(id).bind(key).fetch_optional(&s.db).await? {
        if row.get::<String, _>("request_fingerprint") != fingerprint { return Err(AppError::Conflict("idempotency key was reused with different parameters".into())); }
        let result: PayResult = serde_json::from_value(row.get("response")).map_err(|_| AppError::BadRequest("stored payment response is invalid".into()))?;
        return Ok(Json(result));
    }
    // Steps 1+2 in ONE short transaction (no PSP call inside, no lock held
    // across the PSP call): atomic claim open -> payment_processing plus the
    // attempt row plus the idempotency reservation. A concurrent loser blocks
    // only for microseconds on the row lock, then re-evaluates the WHERE on
    // the new row version and gets 0 rows.
    let mut tx = s.db.begin().await?;
    // Step 1 - claim the invoice. Only one concurrent transaction flips
    // open -> payment_processing; losers match 0 rows.
    let claimed = sqlx::query(
        "UPDATE invoices SET state='payment_processing',updated_at=now() WHERE id=$1 AND business_id=$2 AND state='open' RETURNING total_cents",
    )
    .bind(id)
    .bind(business)
    .fetch_optional(&mut *tx)
    .await?;
    let total_cents = match claimed {
        Some(row) => row.get::<i64, _>("total_cents"),
        None => {
            tx.rollback().await?;
            // Distinguish unknown invoice (404) from unpayable state (409)
            // so cross-tenant / missing ids do not leak as conflicts.
            let exists = sqlx::query("SELECT 1 AS one FROM invoices WHERE id=$1 AND business_id=$2")
                .bind(id)
                .bind(business)
                .fetch_optional(&s.db)
                .await?
                .is_some();
            if !exists {
                return Err(AppError::NotFound);
            }
            // Lost the claim race. Re-check idempotency: a same-key
            // concurrent winner may have committed after our short-circuit.
            if let Some(row) = sqlx::query("SELECT request_fingerprint,response FROM idempotency_keys WHERE business_id=$1 AND invoice_id=$2 AND idempotency_key=$3").bind(business).bind(id).bind(key).fetch_optional(&s.db).await? {
                if row.get::<String, _>("request_fingerprint") != fingerprint { return Err(AppError::Conflict("idempotency key was reused with different parameters".into())); }
                let result: PayResult = serde_json::from_value(row.get("response")).map_err(|_| AppError::BadRequest("stored payment response is invalid".into()))?;
                return Ok(Json(result));
            }
            return Err(AppError::Conflict(
                "invoice is not payable in its current state".into(),
            ));
        }
    };
    // Step 2 - record the payment attempt row. The (business_id, invoice_id,
    // idempotency_key) unique constraint resolves same-key races here.
    let attempt = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO payment_attempts(id,invoice_id,status,card_token) VALUES($1,$2,'pending',$3)",
    )
    .bind(attempt)
    .bind(id)
    .bind(&input.card_token)
    .execute(&mut *tx)
    .await?;
    let pending = PayResult {
        payment_attempt_id: attempt,
        status: "pending".into(),
        failure_code: None,
        psp_reference: None,
    };
    let inserted = sqlx::query("INSERT INTO idempotency_keys(business_id,invoice_id,idempotency_key,request_fingerprint,payment_attempt_id,response) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT (business_id,invoice_id,idempotency_key) DO NOTHING")
        .bind(business)
        .bind(id)
        .bind(key)
        .bind(&fingerprint)
        .bind(attempt)
        .bind(serde_json::to_value(&pending).unwrap_or_default())
        .execute(&mut *tx)
        .await?;
    if inserted.rows_affected() == 0 {
        // Same-key winner committed between our short-circuit and our
        // insert. Roll back our claim + orphan attempt atomically, then
        // return the winner's response. Our attempt never touched the PSP.
        tx.rollback().await?;
        let row = sqlx::query("SELECT request_fingerprint,response FROM idempotency_keys WHERE business_id=$1 AND invoice_id=$2 AND idempotency_key=$3")
            .bind(business).bind(id).bind(key).fetch_one(&s.db).await?;
        if row.get::<String, _>("request_fingerprint") != fingerprint {
            return Err(AppError::Conflict(
                "idempotency key was reused with different parameters".into(),
            ));
        }
        let result: PayResult = serde_json::from_value(row.get("response"))
            .map_err(|_| AppError::BadRequest("stored payment response is invalid".into()))?;
        return Ok(Json(result));
    }
    tx.commit().await?;
    // Step 3 - call the PSP. No lock is held here.
    let response = s.http.post(format!("{}/payments", s.config.psp_base_url)).json(&serde_json::json!({"card_token":input.card_token,"amount_cents":total_cents,"currency":"USD"})).timeout(Duration::from_millis(s.config.psp_timeout_ms)).send().await;
    let (status, failure_code, psp_reference): (String, Option<String>, Option<Uuid>) =
        match response {
            Ok(response) if response.status().is_success() => {
                let body: serde_json::Value =
                    response.json().await.map_err(|_| AppError::External)?;
                match body.get("status").and_then(|v| v.as_str()) {
                    Some("succeeded") => (
                        "succeeded".into(),
                        None,
                        body.get("psp_ref")
                            .and_then(|v| v.as_str())
                            .and_then(|v| Uuid::parse_str(v).ok()),
                    ),
                    Some("failed") => (
                        "failed".into(),
                        body.get("code").and_then(|v| v.as_str()).map(str::to_owned),
                        None,
                    ),
                    _ => ("unknown".into(), Some("invalid_psp_response".into()), None),
                }
            }
            Ok(_) => ("unknown".into(), Some("psp_http_error".into()), None),
            Err(_) => ("unknown".into(), Some("psp_unavailable".into()), None),
        };
    let result = PayResult {
        payment_attempt_id: attempt,
        status: status.clone(),
        failure_code: failure_code.clone(),
        psp_reference,
    };
    let mut tx = s.db.begin().await?;
    sqlx::query("UPDATE payment_attempts SET status=$1,failure_code=$2,psp_reference=$3,updated_at=now() WHERE id=$4").bind(&status).bind(&failure_code).bind(psp_reference).bind(attempt).execute(&mut *tx).await?;
    // Step 4 - finalize with a second conditional update off payment_processing.
    if status == "succeeded" {
        sqlx::query(
            "UPDATE invoices SET state='paid',updated_at=now() WHERE id=$1 AND state='payment_processing'",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
    } else if status == "failed" {
        sqlx::query(
            "UPDATE invoices SET state='open',updated_at=now() WHERE id=$1 AND state='payment_processing'",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
    }
    // Ambiguous (unknown): deliberately leave the invoice in
    // payment_processing. Reverting to open would allow a second request to
    // re-attempt payment while the first PSP call may still succeed
    // asynchronously (tok_timeout), recreating the overcharge risk. Recovery
    // requires reconciliation (see DESIGN.md section 3).
    sqlx::query("UPDATE idempotency_keys SET response=$1 WHERE business_id=$2 AND invoice_id=$3 AND idempotency_key=$4")
        .bind(serde_json::to_value(&result).unwrap_or_default())
        .bind(business)
        .bind(id)
        .bind(key)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    if status == "succeeded" {
        enqueue_event(
            &s.db,
            business,
            "invoice.paid",
            serde_json::json!({"invoice_id":id}),
        )
        .await?;
    } else if status == "failed" {
        enqueue_event(
            &s.db,
            business,
            "invoice.payment_failed",
            serde_json::json!({"invoice_id":id,"code":failure_code}),
        )
        .await?;
    }
    Ok(Json(result))
}

async fn register_endpoint(
    State(s): State<Shared>,
    headers: HeaderMap,
    Json(input): Json<EndpointInput>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let business = auth(&headers, &s).await?;
    if !input.url.starts_with("http://") && !input.url.starts_with("https://") {
        return Err(AppError::BadRequest("url must be http or https".into()));
    }
    let id = Uuid::now_v7();
    let secret = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO webhook_endpoints(id,business_id,url,signing_secret) VALUES($1,$2,$3,$4)",
    )
    .bind(id)
    .bind(business)
    .bind(input.url)
    .bind(&secret)
    .execute(&s.db)
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({"id":id,"signing_secret":secret})),
    ))
}
async fn list_endpoints(
    State(s): State<Shared>,
    headers: HeaderMap,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    let business = auth(&headers, &s).await?;
    let rows=sqlx::query("SELECT id,url,created_at,revoked_at FROM webhook_endpoints WHERE business_id=$1 ORDER BY created_at DESC").bind(business).fetch_all(&s.db).await?;
    Ok(Json(rows.into_iter().map(|r|serde_json::json!({"id":r.get::<Uuid,_>("id"),"url":r.get::<String,_>("url"),"created_at":r.get::<chrono::DateTime<Utc>,_>("created_at"),"revoked_at":r.get::<Option<chrono::DateTime<Utc>>,_>("revoked_at")})).collect()))
}
async fn disable_endpoint(
    State(s): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let business = auth(&headers, &s).await?;
    let result=sqlx::query("UPDATE webhook_endpoints SET revoked_at=now() WHERE id=$1 AND business_id=$2 AND revoked_at IS NULL").bind(id).bind(business).execute(&s.db).await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}
async fn enqueue_event(
    db: &PgPool,
    business: Uuid,
    event_type: &str,
    payload: serde_json::Value,
) -> Result<(), AppError> {
    let rows =
        sqlx::query("SELECT id FROM webhook_endpoints WHERE business_id=$1 AND revoked_at IS NULL")
            .bind(business)
            .fetch_all(db)
            .await?;
    for row in rows {
        sqlx::query("INSERT INTO webhook_deliveries(id,webhook_endpoint_id,event_type,payload,status,next_attempt_at) VALUES($1,$2,$3,$4,'pending',now())").bind(Uuid::now_v7()).bind(row.get::<Uuid,_>("id")).bind(event_type).bind(payload.clone()).execute(db).await?;
    }
    Ok(())
}
async fn webhook_worker(state: Shared) {
    loop {
        let rows = sqlx::query("SELECT d.id,d.attempt_count,d.payload,d.event_type,e.url,e.signing_secret FROM webhook_deliveries d JOIN webhook_endpoints e ON e.id=d.webhook_endpoint_id WHERE d.status='pending' AND d.next_attempt_at <= now() AND e.revoked_at IS NULL ORDER BY d.created_at LIMIT 20")
            .fetch_all(&state.db).await.unwrap_or_default();
        for row in rows {
            let body = row.get::<serde_json::Value, _>("payload");
            let mut mac =
                HmacSha256::new_from_slice(row.get::<String, _>("signing_secret").as_bytes())
                    .unwrap_or_else(|_| {
                        HmacSha256::new_from_slice(b"fallback").expect("constant HMAC key is valid")
                    });
            mac.update(body.to_string().as_bytes());
            let signature = hex(&mac.finalize().into_bytes());
            let response = state
                .http
                .post(row.get::<String, _>("url"))
                .header("X-Webhook-Signature", signature)
                .json(&body)
                .timeout(Duration::from_millis(state.config.webhook_timeout_ms))
                .send()
                .await;
            let id: Uuid = row.get("id");
            let attempts: i32 = row.get("attempt_count");
            if response.map(|r| r.status().is_success()).unwrap_or(false) {
                let _ = sqlx::query("UPDATE webhook_deliveries SET status='delivered',attempt_count=attempt_count+1,last_attempt_at=now(),delivered_at=now() WHERE id=$1").bind(id).execute(&state.db).await;
            } else if attempts + 1 >= state.config.webhook_max_retries {
                let _ = sqlx::query("UPDATE webhook_deliveries SET status='failed',attempt_count=attempt_count+1,last_attempt_at=now() WHERE id=$1").bind(id).execute(&state.db).await;
            } else {
                let delay = 2_i64.pow((attempts + 1).min(10) as u32);
                let _ = sqlx::query("UPDATE webhook_deliveries SET attempt_count=attempt_count+1,last_attempt_at=now(),next_attempt_at=now() + ($1 * interval '1 second') WHERE id=$2").bind(delay).bind(id).execute(&state.db).await;
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
async fn seed_dev_key(db: &PgPool, config: &Config) -> Result<(), sqlx::Error> {
    if sqlx::query("SELECT 1 FROM businesses LIMIT 1")
        .fetch_optional(db)
        .await?
        .is_none()
    {
        let business = Uuid::now_v7();
        sqlx::query("INSERT INTO businesses(id,name) VALUES($1,'Development Business')")
            .bind(business)
            .execute(db)
            .await?;
        let mut mac = HmacSha256::new_from_slice(config.api_key_pepper.as_bytes())
            .expect("HMAC accepts any non-empty configured pepper");
        mac.update(config.dev_api_key_secret.as_bytes());
        sqlx::query("INSERT INTO api_keys(id,business_id,key_id,secret_hash) VALUES($1,$2,$3,$4)")
            .bind(Uuid::now_v7())
            .bind(business)
            .bind(&config.dev_api_key_id)
            .bind(hex(&mac.finalize().into_bytes()))
            .execute(db)
            .await?;
    }
    Ok(())
}
