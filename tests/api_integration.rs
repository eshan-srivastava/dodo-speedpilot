use reqwest::{Client, StatusCode, Url};
use serde::Deserialize;
use serde_json::json;
use std::{env, time::Duration};
use tokio::time::timeout;
use uuid::Uuid;

#[derive(Clone)]
struct TestApi {
    client: Client,
    base_url: Url,
    auth: (String, String),
}

#[derive(Debug, Deserialize)]
struct CustomerResponse {
    id: Uuid,
}

#[derive(Debug, Deserialize)]
struct InvoiceResponse {
    id: Uuid,
    total_cents: i64,
    state: String,
}

#[derive(Debug, Deserialize, PartialEq)]
struct PaymentResponse {
    payment_attempt_id: Uuid,
    status: String,
    failure_code: Option<String>,
    psp_reference: Option<Uuid>,
}

fn api() -> TestApi {
    let base = env::var("TEST_API_BASE_URL")
        .or_else(|_| env::var("API_BASE_URL"))
        .unwrap_or_else(|_| {
            // API_BIND_ADDRESS is the compose service's bind setting. The
            // container port is published on localhost by docker-compose.
            let port = env::var("API_BIND_ADDRESS")
                .ok()
                .and_then(|address| address.rsplit(':').next().map(str::to_owned))
                .unwrap_or_else(|| "8080".into());
            format!("http://127.0.0.1:{port}")
        });
    TestApi {
        client: Client::builder().build().expect("build HTTP client"),
        base_url: Url::parse(&base).expect("TEST_API_BASE_URL must be a valid URL"),
        auth: (
            env::var("DEV_API_KEY_ID").unwrap_or_else(|_| "dev_key".into()),
            env::var("DEV_API_KEY_SECRET").unwrap_or_else(|_| "dev_secret".into()),
        ),
    }
}

impl TestApi {
    async fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.client
            .request(method, self.base_url.join(path).expect("valid API path"))
            .basic_auth(&self.auth.0, Some(&self.auth.1))
    }

    async fn create_invoice(&self) -> InvoiceResponse {
        let customer = self
            .request(reqwest::Method::POST, "/customers")
            .await
            .json(&json!({
                "name": format!("Test customer {}", Uuid::now_v7()),
                "email": format!("{}@example.test", Uuid::now_v7()),
            }))
            .send()
            .await
            .expect("customer request; is invoice-service running?");
        assert_eq!(customer.status(), StatusCode::CREATED);
        let customer: CustomerResponse = customer.json().await.expect("customer JSON");

        let response = self
            .request(reqwest::Method::POST, "/invoices")
            .await
            .json(&json!({
                "customer_id": customer.id,
                "due_date": "2026-09-30",
                "line_items": [
                    {"description": "API development", "quantity": 2, "unit_amount_cents": 15000},
                    {"description": "Hosting", "quantity": 1, "unit_amount_cents": 5000}
                ]
            }))
            .send()
            .await
            .expect("invoice request");
        assert_eq!(response.status(), StatusCode::CREATED);
        response.json().await.expect("invoice JSON")
    }

    async fn get_invoice(&self, id: Uuid) -> InvoiceResponse {
        let response = self
            .request(reqwest::Method::GET, &format!("/invoices/{id}"))
            .await
            .send()
            .await
            .expect("get invoice request");
        assert_eq!(response.status(), StatusCode::OK);
        response.json().await.expect("invoice JSON")
    }

    async fn pay(&self, id: Uuid, key: &str, token: &str) -> reqwest::Response {
        self.request(reqwest::Method::POST, &format!("/invoices/{id}/pay"))
            .await
            .header("Idempotency-Key", key)
            .json(&json!({"card_token": token}))
            .send()
            .await
            .expect("pay request; is mock-psp reachable from invoice-service?")
    }
}

#[tokio::test]
async fn invoice_creation_computes_total_from_line_items() {
    let invoice = api().create_invoice().await;
    assert_eq!(invoice.total_cents, 35_000);
    assert_eq!(invoice.state, "open");
}

#[tokio::test]
async fn pay_succeeds_in_the_ideal_non_concurrent_case() {
    let api = api();
    let invoice = api.create_invoice().await;
    let response = api.pay(invoice.id, "ideal-payment", "tok_success").await;
    assert_eq!(response.status(), StatusCode::OK);
    let payment: PaymentResponse = response.json().await.expect("payment JSON");
    assert_eq!(payment.status, "succeeded");
    assert!(payment.failure_code.is_none());
    assert!(payment.psp_reference.is_some());
    assert_eq!(api.get_invoice(invoice.id).await.state, "paid");
}

#[tokio::test]
async fn concurrent_payments_allow_at_most_one_success() {
    let api = api();
    let invoice = api.create_invoice().await;
    let mut requests = Vec::new();
    for index in 0..8 {
        let api = api.clone();
        let id = invoice.id;
        requests.push(tokio::spawn(async move {
            api.pay(id, &format!("concurrent-{index}"), "tok_success")
                .await
        }));
    }

    let mut successful = 0;
    let mut conflicts = 0;
    for request in requests {
        let response = request.await.expect("concurrent pay task");
        match response.status() {
            StatusCode::OK => {
                successful += 1;
                let payment: PaymentResponse = response.json().await.expect("payment JSON");
                assert_eq!(payment.status, "succeeded");
            }
            StatusCode::CONFLICT => conflicts += 1,
            status => panic!("unexpected concurrent payment status: {status}"),
        }
    }
    assert_eq!(successful, 1);
    assert_eq!(conflicts, 7);
    assert_eq!(api.get_invoice(invoice.id).await.state, "paid");
}

#[tokio::test]
async fn repeated_idempotency_key_replays_the_same_payment() {
    let api = api();
    let invoice = api.create_invoice().await;
    let first = api.pay(invoice.id, "replay-payment", "tok_success").await;
    assert_eq!(first.status(), StatusCode::OK);
    let first: PaymentResponse = first.json().await.expect("first payment JSON");

    let second = api.pay(invoice.id, "replay-payment", "tok_success").await;
    assert_eq!(second.status(), StatusCode::OK);
    let second: PaymentResponse = second.json().await.expect("replayed payment JSON");
    assert_eq!(second, first);
    assert_eq!(api.get_invoice(invoice.id).await.state, "paid");
}

#[tokio::test]
async fn network_failure_is_recorded_as_ambiguous_without_corrupting_invoice() {
    let api = api();
    let invoice = api.create_invoice().await;
    let response = api
        .pay(invoice.id, "network-failure", "tok_network_error")
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let payment: PaymentResponse = response.json().await.expect("payment JSON");
    assert_eq!(payment.status, "unknown");
    assert_eq!(payment.failure_code.as_deref(), Some("psp_http_error"));
    assert_eq!(
        api.get_invoice(invoice.id).await.state,
        "payment_processing"
    );
}

#[tokio::test]
async fn tok_timeout_does_not_hang_the_payment_service() {
    let api = api();
    let invoice = api.create_invoice().await;
    let response = timeout(
        Duration::from_secs(10),
        api.pay(invoice.id, "timeout-payment", "tok_timeout"),
    )
    .await
    .expect("tok_timeout payment exceeded the client-side test bound");
    assert_eq!(response.status(), StatusCode::OK);
    let payment: PaymentResponse = response.json().await.expect("timeout payment JSON");
    assert_eq!(payment.status, "unknown");
    assert_eq!(payment.failure_code.as_deref(), Some("psp_unavailable"));
    assert_eq!(
        api.get_invoice(invoice.id).await.state,
        "payment_processing"
    );
}
