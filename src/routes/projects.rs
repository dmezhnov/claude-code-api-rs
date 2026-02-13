use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde_json::json;

use crate::db;
use crate::error::AppError;
use crate::models::openai::CreateProjectRequest;
use crate::state::AppState;

pub async fn list_projects(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let projects = db::list_projects(&state.db).await?;
    Ok(Json(json!({
        "data": projects,
        "pagination": { "total": projects.len(), "page": 1, "per_page": 20 },
    })))
}

pub async fn create_project(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateProjectRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    let desc = body.description.as_deref().unwrap_or("");
    let project = db::create_project(&state.db, &id, &body.name, desc, body.path.as_deref())
        .await?;
    Ok(Json(serde_json::to_value(project).unwrap_or(json!({}))))
}

pub async fn get_project(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    match db::get_project(&state.db, &project_id).await? {
        Some(p) => Ok(Json(serde_json::to_value(p).unwrap_or(json!({})))),
        None => Err(AppError::NotFound(format!(
            "Project {project_id} not found"
        ))),
    }
}

pub async fn delete_project(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let deleted = db::delete_project(&state.db, &project_id).await?;
    if deleted {
        Ok(Json(json!({
            "project_id": project_id,
            "status": "deleted",
        })))
    } else {
        Err(AppError::NotFound(format!(
            "Project {project_id} not found"
        )))
    }
}
