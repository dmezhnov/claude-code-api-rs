use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub claude_binary_path: String,
    pub database_url: String,
    pub api_keys: Vec<String>,
    pub require_auth: bool,
    pub default_model: String,
    pub max_concurrent_sessions: usize,
    pub session_timeout_minutes: u64,
    pub project_root: PathBuf,
    pub allowed_origins: Vec<String>,
    pub rate_limit_requests_per_minute: u32,
    pub rate_limit_burst: u32,
    pub streaming_timeout_seconds: u64,
    pub cleanup_interval_minutes: u64,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            host: env_or("HOST", "0.0.0.0"),
            port: env_or("PORT", "8000").parse().unwrap_or(8000),
            claude_binary_path: env_or("CLAUDE_BINARY_PATH", "claude"),
            database_url: env_or("DATABASE_URL", "sqlite:./claude_api.db"),
            api_keys: env_csv("API_KEYS"),
            require_auth: env_bool("REQUIRE_AUTH", false),
            default_model: env_or("DEFAULT_MODEL", "claude-3-5-sonnet-20241022"),
            max_concurrent_sessions: env_or("MAX_CONCURRENT_SESSIONS", "10")
                .parse()
                .unwrap_or(10),
            session_timeout_minutes: env_or("SESSION_TIMEOUT_MINUTES", "30")
                .parse()
                .unwrap_or(30),
            project_root: PathBuf::from(env_or(
                "PROJECT_ROOT",
                &std::env::temp_dir().join("claude_projects").to_string_lossy(),
            )),
            allowed_origins: env_csv_or("ALLOWED_ORIGINS", vec!["*".to_string()]),
            rate_limit_requests_per_minute: env_or("RATE_LIMIT_REQUESTS_PER_MINUTE", "100")
                .parse()
                .unwrap_or(100),
            rate_limit_burst: env_or("RATE_LIMIT_BURST", "10")
                .parse()
                .unwrap_or(10),
            streaming_timeout_seconds: env_or("STREAMING_TIMEOUT_SECONDS", "300")
                .parse()
                .unwrap_or(300),
            cleanup_interval_minutes: env_or("CLEANUP_INTERVAL_MINUTES", "60")
                .parse()
                .unwrap_or(60),
        }
    }
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_bool(key: &str, default: bool) -> bool {
    env::var(key)
        .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(default)
}

fn env_csv(key: &str) -> Vec<String> {
    env::var(key)
        .map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default()
}

fn env_csv_or(key: &str, default: Vec<String>) -> Vec<String> {
    let result = env_csv(key);
    if result.is_empty() { default } else { result }
}
