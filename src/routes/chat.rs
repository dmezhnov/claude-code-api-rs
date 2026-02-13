use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde_json::json;

use crate::error::AppError;
use crate::state::AppState;

pub async fn create_chat_completion(
    State(_state): State<Arc<AppState>>,
    Json(_body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    // TODO: implement in Step 5
    Err(AppError::Internal("Not yet implemented".to_string()))
}

pub async fn debug_chat_completion(
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    Json(json!({
        "debug": true,
        "received": body,
    }))
}

pub async fn get_completion_status(
    Path(session_id): Path<String>,
) -> Json<serde_json::Value> {
    Json(json!({
        "session_id": session_id,
        "status": "unknown",
    }))
}

pub async fn stop_completion(
    Path(session_id): Path<String>,
) -> Json<serde_json::Value> {
    Json(json!({
        "session_id": session_id,
        "status": "stopped",
    }))
}
