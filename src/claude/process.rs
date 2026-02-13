use std::pin::Pin;

use futures::Stream;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

use crate::config::Config;
use crate::error::AppError;

/// A running Claude CLI process with streaming JSONL output.
pub struct ClaudeProcess {
    child: Child,
    _temp_dir: Option<tempfile::TempDir>,
}

impl ClaudeProcess {
    /// Spawn a Claude CLI process and return a stream of parsed JSONL messages.
    ///
    /// The prompt is piped via stdin to avoid execve() argument size limits.
    /// Output is streamed line-by-line from stdout using BufReader (true streaming,
    /// unlike the Python version which buffers all output with communicate()).
    pub async fn spawn(
        config: &Config,
        prompt: &str,
        model: &str,
        system_prompt: Option<&str>,
        append_system_prompt: Option<&str>,
        disable_builtin_tools: bool,
    ) -> Result<
        (
            Self,
            Pin<Box<dyn Stream<Item = serde_json::Value> + Send>>,
            Option<String>,
        ),
        AppError,
    > {
        let mut cmd = Command::new(&config.claude_binary_path);
        cmd.arg("-p");

        let mut temp_dir = None;

        // Handle system prompt: write to CLAUDE.md if large (>10KB)
        if let Some(sp) = system_prompt {
            if sp.len() > 10_000 {
                let dir = tempfile::tempdir().map_err(|e| {
                    AppError::Internal(format!("Failed to create temp dir: {e}"))
                })?;
                let claude_md = dir.path().join("CLAUDE.md");
                tokio::fs::write(&claude_md, sp).await.map_err(|e| {
                    AppError::Internal(format!("Failed to write CLAUDE.md: {e}"))
                })?;
                tracing::info!(
                    path = %claude_md.display(),
                    size = sp.len(),
                    "System prompt written to CLAUDE.md"
                );
                cmd.current_dir(dir.path());
                temp_dir = Some(dir);
            } else {
                cmd.args(["--system-prompt", sp]);
            }
        }

        if let Some(asp) = append_system_prompt {
            cmd.args(["--append-system-prompt", asp]);
        }

        if disable_builtin_tools {
            cmd.args(["--tools", ""]);
        }

        cmd.args(["--model", model]);
        cmd.args(["--output-format", "stream-json"]);
        cmd.args(["--verbose", "--dangerously-skip-permissions"]);
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        tracing::info!(
            model,
            prompt_size = prompt.len(),
            system_prompt_size = system_prompt.map(|s| s.len()).unwrap_or(0),
            "Spawning Claude process"
        );

        let mut child = cmd.spawn().map_err(|e| {
            AppError::ServiceUnavailable(format!("Failed to spawn Claude: {e}"))
        })?;

        // Pipe prompt through stdin
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(prompt.as_bytes()).await.map_err(|e| {
                AppError::Internal(format!("Failed to write prompt to stdin: {e}"))
            })?;
            // Drop stdin to signal EOF — Claude will start processing
            drop(stdin);
        }

        // Create streaming reader from stdout
        let stdout = child.stdout.take().ok_or_else(|| {
            AppError::Internal("Failed to capture stdout".to_string())
        })?;

        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        // Extract session_id from first message, then yield all messages
        let (tx, rx) = tokio::sync::mpsc::channel::<serde_json::Value>(64);
        let mut session_id_holder: Option<String> = None;

        // Read first line to extract session_id
        if let Ok(Some(first_line)) = lines.next_line().await {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&first_line) {
                if let Some(sid) = val.get("session_id").and_then(|v| v.as_str()) {
                    session_id_holder = Some(sid.to_string());
                }
                let _ = tx.send(val).await;
            }
        }

        let session_id = session_id_holder.clone();

        // Spawn task to read remaining lines and send them through the channel
        tokio::spawn(async move {
            while let Ok(Some(line)) = lines.next_line().await {
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<serde_json::Value>(&line) {
                    Ok(val) => {
                        if tx.send(val).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => {
                        // Non-JSON output
                        let val = serde_json::json!({"type": "text", "content": line});
                        if tx.send(val).await.is_err() {
                            break;
                        }
                    }
                }
            }
        });

        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);

        Ok((
            Self {
                child,
                _temp_dir: temp_dir,
            },
            Box::pin(stream),
            session_id,
        ))
    }

    /// Kill the subprocess.
    pub async fn kill(&mut self) {
        let _ = self.child.kill().await;
    }
}
