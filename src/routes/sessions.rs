use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde_json::json;

use crate::db;
use crate::error::AppError;
use crate::models::openai::CreateSessionRequest;
use crate::state::AppState;

pub async fn list_sessions(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let sessions = db::list_sessions(&state.db).await?;
    Ok(Json(json!({
        "data": sessions,
        "pagination": { "total": sessions.len(), "page": 1, "per_page": 20 },
    })))
}

pub async fn create_session(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateSessionRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    let model = body
        .model
        .as_deref()
        .unwrap_or(&state.config.default_model);
    let session = db::create_session(
        &state.db,
        &id,
        Some(&body.project_id),
        model,
        body.system_prompt.as_deref(),
        body.title.as_deref(),
    )
    .await?;
    Ok(Json(serde_json::to_value(session).unwrap_or(json!({}))))
}

pub async fn get_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    match db::get_session(&state.db, &session_id).await? {
        Some(s) => Ok(Json(serde_json::to_value(s).unwrap_or(json!({})))),
        None => Err(AppError::NotFound(format!(
            "Session {session_id} not found"
        ))),
    }
}

pub async fn delete_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let deleted = db::delete_session(&state.db, &session_id).await?;
    if deleted {
        Ok(Json(json!({
            "session_id": session_id,
            "status": "deleted",
        })))
    } else {
        Err(AppError::NotFound(format!(
            "Session {session_id} not found"
        )))
    }
}

pub async fn get_session_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let active_count = state.claude_manager.active_count().await;
    let active_ids = state.claude_manager.active_session_ids().await;

    Ok(Json(json!({
        "active_claude_sessions": active_count,
        "claude_sessions": active_ids,
    })))
}
