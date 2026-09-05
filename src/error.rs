use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
pub struct ErrorBody {
    pub error: ErrorDetail,
}
#[derive(Serialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    pub request_id: String,
}

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("authentication failed")]
    Unauthorized,
    #[error("resource not found")]
    NotFound,
    #[error("{0}")]
    BadRequest(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("internal server error")]
    Internal(#[from] sqlx::Error),
    #[error("external payment provider error")]
    External,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "authentication_failed",
                "authentication failed".into(),
            ),
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                "not_found",
                "resource not found".into(),
            ),
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, "validation_error", message),
            Self::Conflict(message) => (
                StatusCode::CONFLICT,
                "conflict",
                format!("conflict: {message}"),
            ),
            Self::External => (
                StatusCode::BAD_GATEWAY,
                "external_provider_error",
                "external payment provider error".into(),
            ),
            Self::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "internal server error".into(),
            ),
        };
        (
            status,
            Json(ErrorBody {
                error: ErrorDetail {
                    code: code.into(),
                    message,
                    request_id: Uuid::now_v7().to_string(),
                },
            }),
        )
            .into_response()
    }
}
