pub mod root;
pub mod chat;
pub mod embeddings;
pub mod models;
pub mod projects;
pub mod sessions;

use std::sync::Arc;

use axum::routing::{delete, get, post};
use axum::Router;

use crate::state::AppState;

pub fn build_router(state: Arc<AppState>) -> Router {
    let v1 = Router::new()
        // Chat completions
        .route("/chat/completions", post(chat::create_chat_completion))
        .route("/chat/completions/debug", post(chat::debug_chat_completion))
        .route(
            "/chat/completions/{session_id}/status",
            get(chat::get_completion_status),
        )
        .route(
            "/chat/completions/{session_id}",
            delete(chat::stop_completion),
        )
        // Embeddings
        .route("/embeddings", post(embeddings::create_embeddings))
        // Models
        .route("/models", get(models::list_models))
        .route("/models/capabilities", get(models::get_model_capabilities))
        .route("/models/{model_id}", get(models::get_model))
        // Projects
        .route("/projects", get(projects::list_projects).post(projects::create_project))
        .route(
            "/projects/{project_id}",
            get(projects::get_project).delete(projects::delete_project),
        )
        // Sessions
        .route("/sessions", get(sessions::list_sessions).post(sessions::create_session))
        .route("/sessions/stats", get(sessions::get_session_stats))
        .route(
            "/sessions/{session_id}",
            get(sessions::get_session).delete(sessions::delete_session),
        );

    Router::new()
        .route("/", get(root::root))
        .route("/health", get(root::health))
        .nest("/v1", v1)
        .with_state(state)
}
