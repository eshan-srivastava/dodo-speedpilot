use axum::{Json, Router, routing::post};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

#[derive(Deserialize)]
struct Request {
    card_token: String,
}
#[derive(Serialize)]
struct Response {
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    psp_ref: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new().route("/payments", post(pay));
    let listener = tokio::net::TcpListener::bind(
        std::env::var("PSP_BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:8081".into()),
    )
    .await?;
    axum::serve(listener, app).await?;
    Ok(())
}
async fn pay(Json(req): Json<Request>) -> (axum::http::StatusCode, Json<Response>) {
    match req.card_token.as_str() {
        "tok_success" => {
            tokio::time::sleep(Duration::from_millis(100)).await;
            (
                axum::http::StatusCode::OK,
                Json(Response {
                    status: "succeeded".into(),
                    psp_ref: Some(Uuid::now_v7()),
                    code: None,
                }),
            )
        }
        "tok_timeout" => {
            tokio::time::sleep(Duration::from_secs(30)).await;
            (
                axum::http::StatusCode::OK,
                Json(Response {
                    status: "succeeded".into(),
                    psp_ref: Some(Uuid::now_v7()),
                    code: None,
                }),
            )
        }
        "tok_insufficient_funds" => failure("insufficient_funds").await,
        "tok_card_declined" => failure("card_declined").await,
        "tok_network_error" => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(Response {
                status: "failed".into(),
                psp_ref: None,
                code: Some("network_error".into()),
            }),
        ),
        _ => failure("unknown_token").await,
    }
}
async fn failure(code: &str) -> (axum::http::StatusCode, Json<Response>) {
    tokio::time::sleep(Duration::from_millis(100)).await;
    (
        axum::http::StatusCode::OK,
        Json(Response {
            status: "failed".into(),
            psp_ref: None,
            code: Some(code.into()),
        }),
    )
}
