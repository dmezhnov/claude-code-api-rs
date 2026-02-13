use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::RwLock;

use crate::auth::RateLimiter;
use crate::claude::manager::ClaudeManager;
use crate::config::Config;

pub struct AppState {
    pub config: Config,
    pub db: SqlitePool,
    pub rate_limiter: RwLock<RateLimiter>,
    pub claude_manager: ClaudeManager,
}

impl AppState {
    pub fn new(config: Config, db: SqlitePool) -> Arc<Self> {
        let rate_limiter = RwLock::new(RateLimiter::new(
            config.rate_limit_requests_per_minute,
            config.rate_limit_burst,
        ));
        let claude_manager = ClaudeManager::new(config.clone());
        Arc::new(Self {
            config,
            db,
            rate_limiter,
            claude_manager,
        })
    }
}
