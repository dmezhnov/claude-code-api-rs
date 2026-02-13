use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde_json::json;

use crate::error::AppError;
use crate::state::AppState;

pub async fn list_sessions(
    State(_state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    // TODO: implement DB query in Step 3
    Json(json!({
        "data": [],
        "pagination": { "total": 0, "page": 1, "per_page": 20 },
    }))
}

pub async fn create_session(
    State(_state): State<Arc<AppState>>,
    Json(_body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    Err(AppError::Internal("Not yet implemented".to_string()))
}

pub async fn get_session(
    State(_state): State<Arc<AppState>>,
    Path(_session_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    Err(AppError::NotFound("Session not found".to_string()))
}

pub async fn delete_session(
    Path(session_id): Path<String>,
) -> Json<serde_json::Value> {
    Json(json!({
        "session_id": session_id,
        "status": "deleted",
    }))
}

pub async fn get_session_stats(
    State(_state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    Json(json!({
        "session_stats": {
            "active_sessions": 0,
            "total_tokens": 0,
            "total_cost": 0.0,
            "total_messages": 0,
            "models_in_use": [],
        },
        "active_claude_sessions": 0,
        "claude_sessions": [],
    }))
}
