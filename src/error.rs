use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    BadRequest(String),
    Unauthorized(String),
    NotFound(String),
    RateLimited,
    ServiceUnavailable(String),
    Internal(String),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadRequest(msg) => write!(f, "Bad request: {msg}"),
            Self::Unauthorized(msg) => write!(f, "Unauthorized: {msg}"),
            Self::NotFound(msg) => write!(f, "Not found: {msg}"),
            Self::RateLimited => write!(f, "Rate limit exceeded"),
            Self::ServiceUnavailable(msg) => write!(f, "Service unavailable: {msg}"),
            Self::Internal(msg) => write!(f, "Internal error: {msg}"),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_type, code, message) = match &self {
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, "invalid_request_error", "bad_request", msg.clone()),
            Self::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "authentication_error", "invalid_api_key", msg.clone()),
            Self::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found", "not_found", msg.clone()),
            Self::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "rate_limit_error", "rate_limit_exceeded", "Rate limit exceeded".to_string()),
            Self::ServiceUnavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, "service_error", "service_unavailable", msg.clone()),
            Self::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error", "internal_error", msg.clone()),
        };

        let body = json!({
            "error": {
                "message": message,
                "type": error_type,
                "code": code,
            }
        });

        (status, Json(body)).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!(error = %e, "Database error");
        Self::Internal("Database error".to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        tracing::error!(error = %e, "IO error");
        Self::Internal(format!("IO error: {e}"))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        Self::BadRequest(format!("JSON error: {e}"))
    }
}
