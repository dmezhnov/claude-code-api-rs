mod auth;
mod claude;
mod config;
mod db;
mod error;
mod models;
mod routes;
mod state;
mod streaming;
mod tools;

use std::net::SocketAddr;

use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::prelude::*;

use crate::config::Config;
use crate::state::AppState;

#[tokio::main]
async fn main() {
    // Load .env file if present
    let _ = dotenvy::dotenv();

    // Initialize structured logging (JSON)
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    // Load configuration
    let config = Config::from_env();
    let addr = SocketAddr::new(
        config.host.parse().expect("Invalid HOST"),
        config.port,
    );

    tracing::info!(
        host = %config.host,
        port = config.port,
        claude_binary = %config.claude_binary_path,
        "Starting Claude Code API Gateway (Rust)"
    );

    // Initialize database
    let db = db::init_db(&config.database_url)
        .await
        .expect("Failed to initialize database");
    tracing::info!("Database initialized");

    // Build shared state
    let state = AppState::new(config, db);

    // Build CORS layer
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build router
    let app = routes::build_router(state.clone())
        .layer(axum::middleware::from_fn_with_state(
            state,
            auth::auth_middleware,
        ))
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    // Start server
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind");
    tracing::info!(%addr, "Server listening");

    // Graceful shutdown on ctrl-c
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Server error");

    tracing::info!("Server shut down");
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for ctrl-c");
    tracing::info!("Shutdown signal received");
}
