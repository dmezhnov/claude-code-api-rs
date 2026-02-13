use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::RwLock;

use crate::auth::RateLimiter;
use crate::config::Config;

pub struct AppState {
    pub config: Config,
    pub db: SqlitePool,
    pub rate_limiter: RwLock<RateLimiter>,
}

impl AppState {
    pub fn new(config: Config, db: SqlitePool) -> Arc<Self> {
        let rate_limiter = RwLock::new(RateLimiter::new(
            config.rate_limit_requests_per_minute,
            config.rate_limit_burst,
        ));
        Arc::new(Self {
            config,
            db,
            rate_limiter,
        })
    }
}
