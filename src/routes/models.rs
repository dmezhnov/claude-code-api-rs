use axum::extract::Path;
use axum::Json;
use serde_json::json;

use crate::error::AppError;

pub async fn list_models() -> Json<serde_json::Value> {
    Json(json!({
        "object": "list",
        "data": get_model_objects(),
    }))
}

pub async fn get_model(
    Path(model_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let models = get_model_objects();
    let model = models
        .iter()
        .find(|m| m["id"].as_str() == Some(model_id.as_str()));

    match model {
        Some(m) => Ok(Json(m.clone())),
        None => Err(AppError::NotFound(format!("Model '{model_id}' not found"))),
    }
}

pub async fn get_model_capabilities() -> Json<serde_json::Value> {
    // TODO: detailed capabilities in Step 2
    Json(json!({
        "models": get_model_objects(),
    }))
}

fn get_model_objects() -> Vec<serde_json::Value> {
    vec![
        json!({
            "id": "claude-opus-4-6",
            "object": "model",
            "created": 1700000000,
            "owned_by": "anthropic",
        }),
        json!({
            "id": "cc-sonnet-45",
            "object": "model",
            "created": 1700000000,
            "owned_by": "anthropic",
        }),
        json!({
            "id": "cc-haiku-45",
            "object": "model",
            "created": 1700000000,
            "owned_by": "anthropic",
        }),
        json!({
            "id": "claude-3-7-sonnet-20250219",
            "object": "model",
            "created": 1700000000,
            "owned_by": "anthropic",
        }),
    ]
}
