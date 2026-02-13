use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use futures::Stream;
use tokio::sync::RwLock;

use crate::claude::process::ClaudeProcess;
use crate::config::Config;
use crate::error::AppError;

/// Manages concurrent Claude CLI processes.
pub struct ClaudeManager {
    config: Config,
    active: Arc<RwLock<HashMap<String, ClaudeProcess>>>,
    max_concurrent: usize,
}

impl ClaudeManager {
    pub fn new(config: Config) -> Self {
        let max = config.max_concurrent_sessions;
        Self {
            config,
            active: Arc::new(RwLock::new(HashMap::new())),
            max_concurrent: max,
        }
    }

    /// Spawn a Claude CLI process and return the JSONL stream.
    ///
    /// The process is tracked for concurrent-session limiting and can be
    /// killed via [`stop_session`].
    pub async fn create_session(
        &self,
        session_id: &str,
        prompt: &str,
        model: &str,
        system_prompt: Option<&str>,
        append_system_prompt: Option<&str>,
        disable_builtin_tools: bool,
    ) -> Result<
        (
            Pin<Box<dyn Stream<Item = serde_json::Value> + Send>>,
            Option<String>,
        ),
        AppError,
    > {
        let count = self.active.read().await.len();
        if count >= self.max_concurrent {
            return Err(AppError::ServiceUnavailable(format!(
                "Maximum concurrent sessions ({}) reached",
                self.max_concurrent
            )));
        }

        let (process, stream, claude_sid) = ClaudeProcess::spawn(
            &self.config,
            prompt,
            model,
            system_prompt,
            append_system_prompt,
            disable_builtin_tools,
        )
        .await?;

        let key = claude_sid
            .clone()
            .unwrap_or_else(|| session_id.to_string());
        self.active.write().await.insert(key, process);

        Ok((stream, claude_sid))
    }

    /// Kill a running session by its ID.
    pub async fn stop_session(&self, session_id: &str) {
        if let Some(mut process) = self.active.write().await.remove(session_id) {
            process.kill().await;
            tracing::info!(session_id, "Claude session stopped");
        }
    }

    /// Remove a finished session from tracking and reap the child process.
    pub async fn session_finished(&self, session_id: &str) {
        if let Some(mut process) = self.active.write().await.remove(session_id) {
            process.reap().await;
        }
    }

    /// Number of currently active sessions.
    pub async fn active_count(&self) -> usize {
        self.active.read().await.len()
    }

    /// List active session IDs.
    pub async fn active_session_ids(&self) -> Vec<String> {
        self.active.read().await.keys().cloned().collect()
    }

    /// Stop all sessions and reap all child processes.
    pub async fn cleanup_all(&self) {
        let mut map = self.active.write().await;
        for (sid, mut process) in map.drain() {
            process.kill().await;
            tracing::info!(session_id = %sid, "Session cleaned up");
        }
    }

    /// Log how many active sessions remain (useful for debugging leaks).
    pub async fn debug_sessions(&self) {
        let map = self.active.read().await;
        if !map.is_empty() {
            let ids: Vec<_> = map.keys().collect();
            tracing::warn!(
                count = map.len(),
                sessions = ?ids,
                "Active sessions still tracked"
            );
        }
    }
}

/// Create a project directory under `project_root`.
pub fn create_project_directory(
    project_root: &std::path::Path,
    project_id: &str,
) -> std::path::PathBuf {
    let path = project_root.join(project_id);
    let _ = std::fs::create_dir_all(&path);
    path
}
