use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde_json::json;

use crate::error::AppError;
use crate::state::AppState;

pub async fn list_projects(
    State(_state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    // TODO: implement DB query in Step 3
    Json(json!({
        "data": [],
        "pagination": { "total": 0, "page": 1, "per_page": 20 },
    }))
}

pub async fn create_project(
    State(_state): State<Arc<AppState>>,
    Json(_body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    Err(AppError::Internal("Not yet implemented".to_string()))
}

pub async fn get_project(
    State(_state): State<Arc<AppState>>,
    Path(_project_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    Err(AppError::NotFound("Project not found".to_string()))
}

pub async fn delete_project(
    Path(project_id): Path<String>,
) -> Json<serde_json::Value> {
    Json(json!({
        "project_id": project_id,
        "status": "deleted",
    }))
}
