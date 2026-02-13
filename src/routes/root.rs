use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;

use crate::state::AppState;

pub async fn root() -> Json<serde_json::Value> {
    Json(json!({
        "name": "Claude Code API Gateway",
        "version": "1.0.0",
        "description": "OpenAI-compatible API for Claude Code",
        "backend": "rust-axum",
        "endpoints": {
            "chat": "/v1/chat/completions",
            "models": "/v1/models",
            "projects": "/v1/projects",
            "sessions": "/v1/sessions",
        },
        "docs": "/docs",
        "health": "/health",
    }))
}

pub async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match get_claude_version(&state.config.claude_binary_path).await {
        Ok(version) => Json(json!({
            "status": "healthy",
            "version": "1.0.0",
            "backend": "rust-axum",
            "claude_version": version,
            "active_sessions": 0,
        }))
        .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Health check failed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "status": "unhealthy",
                    "error": e.to_string(),
                })),
            )
                .into_response()
        }
    }
}

async fn get_claude_version(binary: &str) -> Result<String, std::io::Error> {
    let output = tokio::process::Command::new(binary)
        .arg("--version")
        .output()
        .await?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!(
                "Claude version check failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ))
    }
}
