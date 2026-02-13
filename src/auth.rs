use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::state::AppState;

/// Sliding-window rate limiter per API key.
pub struct RateLimiter {
    requests_per_minute: u32,
    burst: u32,
    windows: HashMap<String, Vec<Instant>>,
}

impl RateLimiter {
    pub fn new(requests_per_minute: u32, burst: u32) -> Self {
        Self {
            requests_per_minute,
            burst,
            windows: HashMap::new(),
        }
    }

    pub fn check(&mut self, key: &str) -> bool {
        let now = Instant::now();
        let window = self.windows.entry(key.to_string()).or_default();

        // Remove entries older than 60 seconds
        window.retain(|t| now.duration_since(*t).as_secs() < 60);

        if window.len() as u32 >= self.requests_per_minute + self.burst {
            return false;
        }

        window.push(now);
        true
    }
}

/// Extract API key from request headers or query string.
pub fn extract_api_key(headers: &HeaderMap, query: &str) -> Option<String> {
    // Check Authorization: Bearer <key>
    if let Some(auth) = headers.get("authorization") {
        if let Ok(val) = auth.to_str() {
            if let Some(key) = val.strip_prefix("Bearer ") {
                return Some(key.trim().to_string());
            }
        }
    }

    // Check x-api-key header
    if let Some(key) = headers.get("x-api-key") {
        if let Ok(val) = key.to_str() {
            return Some(val.trim().to_string());
        }
    }

    // Check api_key query parameter
    for pair in query.split('&') {
        if let Some(val) = pair.strip_prefix("api_key=") {
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }

    None
}

fn validate_api_key(key: &str, valid_keys: &[String]) -> bool {
    valid_keys.iter().any(|k| k == key)
}

const PUBLIC_PATHS: &[&str] = &["/", "/health", "/docs", "/redoc", "/openapi.json"];

/// Authentication and rate-limiting middleware.
pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();

    // Skip auth for public paths
    if PUBLIC_PATHS.iter().any(|p| path == *p) {
        return next.run(req).await;
    }

    // If auth not required, pass through
    if !state.config.require_auth {
        return next.run(req).await;
    }

    let query = req.uri().query().unwrap_or("");
    let api_key = extract_api_key(req.headers(), query);

    let Some(key) = api_key else {
        return error_response(
            StatusCode::UNAUTHORIZED,
            "authentication_error",
            "missing_api_key",
            "Missing API key. Provide via Authorization: Bearer <key>, x-api-key header, or api_key query param.",
        );
    };

    if !validate_api_key(&key, &state.config.api_keys) {
        return error_response(
            StatusCode::UNAUTHORIZED,
            "authentication_error",
            "invalid_api_key",
            "Invalid API key",
        );
    }

    // Rate limiting
    {
        let mut limiter = state.rate_limiter.write().await;
        if !limiter.check(&key) {
            return error_response(
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limit_error",
                "rate_limit_exceeded",
                "Rate limit exceeded",
            );
        }
    }

    next.run(req).await
}

fn error_response(status: StatusCode, error_type: &str, code: &str, message: &str) -> Response {
    let body = json!({
        "error": {
            "message": message,
            "type": error_type,
            "code": code,
        }
    });
    (status, axum::Json(body)).into_response()
}
