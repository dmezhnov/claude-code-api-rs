use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures::StreamExt;
use serde_json::json;

use crate::claude::manager::create_project_directory;
use crate::claude::parser::{
    extract_assistant_content, extract_usage, is_assistant_message, is_result_message,
};
use crate::db;
use crate::error::AppError;
use crate::models::claude::validate_claude_model;
use crate::models::openai::{
    ChatCompletionChoice, ChatCompletionRequest, ChatCompletionResponse, ChatCompletionUsage,
    ChatMessageResponse,
};
use crate::state::AppState;
use crate::streaming;
use crate::tools::{format_tools_prompt, parse_tool_calls};

pub async fn create_chat_completion(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ChatCompletionRequest>,
) -> Result<Response, AppError> {
    // When tools are present, collect full response for tool_call parsing
    let has_tools = request.tools.as_ref().map_or(false, |t| !t.is_empty());
    let wants_stream = request.stream.unwrap_or(false);
    let do_stream = wants_stream && !has_tools;

    // Validate / resolve model alias
    let claude_model = validate_claude_model(&request.model);

    // Must have at least one user message
    if request.messages.is_empty() {
        return Err(AppError::BadRequest(
            "At least one message is required".to_string(),
        ));
    }
    let user_messages: Vec<_> = request
        .messages
        .iter()
        .filter(|m| m.role == "user")
        .collect();
    if user_messages.is_empty() {
        return Err(AppError::BadRequest(
            "At least one user message is required".to_string(),
        ));
    }

    // Build conversation prompt from messages.
    // Skip the first system message (extracted as system_prompt),
    // but keep subsequent system messages as [System Event] in history.
    let conversation_messages: Vec<_> = {
        let mut first_system_seen = false;
        request
            .messages
            .iter()
            .filter(|msg| {
                if msg.role == "system" {
                    if first_system_seen {
                        true
                    } else {
                        first_system_seen = true;
                        false
                    }
                } else {
                    true
                }
            })
            .collect()
    };

    let last_user = user_messages.last().unwrap();
    let user_prompt = if conversation_messages.len() > 1 {
        let parts: Vec<String> = conversation_messages
            .iter()
            .map(|msg| match msg.role.as_str() {
                "user" => format!("[User]: {}", msg.get_text_content()),
                "assistant" => {
                    let mut text = msg.get_text_content();
                    if let Some(ref tcs) = msg.tool_calls {
                        for tc in tcs {
                            text.push_str(&format!(
                                "\n[Called tool: {}({})]",
                                tc.function.name, tc.function.arguments
                            ));
                        }
                    }
                    format!("[Assistant]: {text}")
                }
                "system" => format!("[System Event]: {}", msg.get_text_content()),
                "tool" => {
                    let name = msg.name.as_deref().unwrap_or("unknown");
                    format!("[Tool Result ({name})]: {}", msg.get_text_content())
                }
                _ => format!("[{}]: {}", msg.role, msg.get_text_content()),
            })
            .collect();

        format!(
            "Below is the conversation history. Continue naturally from where it left off. \
             Reply ONLY as the Assistant to the last User message.\n\n{}",
            parts.join("\n\n")
        )
    } else {
        last_user.get_text_content()
    };

    // Handle vision: extract images and prepend Read instructions
    let image_paths = last_user.extract_images();
    let user_prompt = if !image_paths.is_empty() {
        let refs: Vec<String> = image_paths
            .iter()
            .enumerate()
            .map(|(i, p)| format!("- Image {}: {p}", i + 1))
            .collect();
        format!(
            "Read the following image file(s) using the Read tool, \
             then answer the question below.\n\n\
             Image files:\n{}\n\n\
             Question: {user_prompt}",
            refs.join("\n")
        )
    } else {
        user_prompt
    };

    // Extract system prompt (first system message)
    let system_prompt = request
        .messages
        .iter()
        .find(|m| m.role == "system")
        .map(|m| m.get_text_content())
        .or_else(|| request.system_prompt.clone());

    // Build tool prompt appendix
    let append_system_prompt = if has_tools {
        Some(format_tools_prompt(request.tools.as_deref().unwrap_or(&[])))
    } else {
        None
    };

    // Project context
    let project_id = request
        .project_id
        .clone()
        .unwrap_or_else(|| "default".to_string());
    let _project_path = create_project_directory(&state.config.project_root, &project_id);

    // Session management
    let session_id = request
        .session_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    tracing::info!(
        model = %claude_model,
        prompt_size = user_prompt.len(),
        stream = do_stream,
        has_tools,
        session_id = %session_id,
        "Chat completion request"
    );

    // Spawn Claude process
    let (claude_stream, claude_session_id) = state
        .claude_manager
        .create_session(
            &session_id,
            &user_prompt,
            &claude_model,
            system_prompt.as_deref(),
            append_system_prompt.as_deref(),
            has_tools,
        )
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to create Claude session");
            AppError::ServiceUnavailable(format!("Failed to start Claude Code: {e}"))
        })?;

    let effective_session_id = claude_session_id
        .clone()
        .unwrap_or_else(|| session_id.clone());

    // Save user message to DB (fire-and-forget)
    let db = state.db.clone();
    let sid = effective_session_id.clone();
    let prompt_clone = user_prompt.clone();
    tokio::spawn(async move {
        let _ = db::add_message(&db, &sid, "user", &prompt_clone, 0, 0, 0.0).await;
    });

    // ── Streaming path ──
    if do_stream {
        let completion_id = format!(
            "chatcmpl-{}",
            &uuid::Uuid::new_v4().as_simple().to_string()[..29]
        );
        let created = chrono::Utc::now().timestamp();
        let model = claude_model.to_string();
        let state_clone = Arc::clone(&state);
        let sid = effective_session_id.clone();

        let (tx, rx) = tokio::sync::mpsc::channel::<String>(64);

        tokio::spawn(async move {
            let _ = tx
                .send(streaming::sse_event(&streaming::initial_chunk(
                    &completion_id,
                    &model,
                    created,
                )))
                .await;

            let mut claude_stream = claude_stream;
            while let Some(msg) = claude_stream.next().await {
                if is_assistant_message(&msg) {
                    if let Some(content) = extract_assistant_content(&msg) {
                        let _ = tx
                            .send(streaming::sse_event(&streaming::content_chunk(
                                &completion_id,
                                &model,
                                created,
                                &content,
                            )))
                            .await;
                    }
                }
                if is_result_message(&msg) {
                    if let Some(usage) = extract_usage(&msg) {
                        let _ = db::update_session_metrics(
                            &state_clone.db,
                            &sid,
                            (usage.input_tokens + usage.output_tokens) as i64,
                            usage.cost_usd,
                        )
                        .await;
                    }
                    break;
                }
            }

            let _ = tx
                .send(streaming::sse_event(&streaming::final_chunk(
                    &completion_id,
                    &model,
                    created,
                    "stop",
                )))
                .await;
            let _ = tx.send(streaming::sse_done()).await;

            state_clone.claude_manager.session_finished(&sid).await;
        });

        let body_stream =
            tokio_stream::wrappers::ReceiverStream::new(rx).map(Ok::<_, std::io::Error>);
        let body = Body::from_stream(body_stream);

        return Ok(Response::builder()
            .status(200)
            .header("Content-Type", "text/event-stream")
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive")
            .header("X-Session-ID", &effective_session_id)
            .header("X-Project-ID", &project_id)
            .body(body)
            .unwrap()
            .into_response());
    }

    // ── Non-streaming path ──
    {
        let mut claude_stream = claude_stream;
        let mut content_parts = Vec::new();
        let mut usage_input: u32 = 0;
        let mut usage_output: u32 = 0;
        let mut cost: f64 = 0.0;

        while let Some(msg) = claude_stream.next().await {
            if is_assistant_message(&msg) {
                if let Some(text) = extract_assistant_content(&msg) {
                    content_parts.push(text);
                }
            }
            if is_result_message(&msg) {
                if let Some(u) = extract_usage(&msg) {
                    usage_input = u.input_tokens;
                    usage_output = u.output_tokens;
                    cost = u.cost_usd;
                }
                break;
            }
        }

        state
            .claude_manager
            .session_finished(&effective_session_id)
            .await;

        let complete_content = if content_parts.is_empty() {
            "Hello! I'm Claude, ready to help.".to_string()
        } else {
            content_parts.join("\n")
        };

        // Parse tool calls from response text
        let (tool_calls, cleaned_text) = if has_tools {
            parse_tool_calls(&complete_content)
        } else {
            (None, complete_content.clone())
        };

        let (response_content, response_tool_calls, finish_reason) = if tool_calls.is_some() {
            // Drop text content when tool_calls are present to avoid duplicate messages
            (None, tool_calls, "tool_calls".to_string())
        } else {
            (Some(cleaned_text), None, "stop".to_string())
        };

        let completion_id = format!(
            "chatcmpl-{}",
            &uuid::Uuid::new_v4().as_simple().to_string()[..29]
        );
        let created = chrono::Utc::now().timestamp();

        let response = ChatCompletionResponse {
            id: completion_id,
            object: "chat.completion".to_string(),
            created,
            model: claude_model.to_string(),
            choices: vec![ChatCompletionChoice {
                index: 0,
                message: ChatMessageResponse {
                    role: "assistant".to_string(),
                    content: response_content,
                    tool_calls: response_tool_calls,
                },
                finish_reason,
            }],
            usage: ChatCompletionUsage {
                prompt_tokens: usage_input,
                completion_tokens: usage_output,
                total_tokens: usage_input + usage_output,
            },
            session_id: Some(effective_session_id.clone()),
            project_id: Some(project_id.clone()),
        };

        // Save assistant message to DB
        let _ = db::add_message(
            &state.db,
            &effective_session_id,
            "assistant",
            &complete_content,
            usage_input as i64,
            usage_output as i64,
            cost,
        )
        .await;
        let _ = db::update_session_metrics(
            &state.db,
            &effective_session_id,
            (usage_input + usage_output) as i64,
            cost,
        )
        .await;

        // If the client originally requested streaming, wrap as SSE
        if wants_stream {
            let response_value = serde_json::to_value(&response)?;
            let events = streaming::wrap_response_as_sse(&response_value);
            let all_events = events.join("");

            let body = Body::from(all_events);
            return Ok(Response::builder()
                .status(200)
                .header("Content-Type", "text/event-stream")
                .header("Cache-Control", "no-cache")
                .header("Connection", "keep-alive")
                .header("X-Session-ID", &effective_session_id)
                .body(body)
                .unwrap()
                .into_response());
        }

        Ok(Json(response).into_response())
    }
}

pub async fn debug_chat_completion(
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    Json(json!({
        "debug": true,
        "received": body,
    }))
}

pub async fn get_completion_status(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    match db::get_session(&state.db, &session_id).await? {
        Some(s) => Ok(Json(json!({
            "session_id": session_id,
            "model": s.model,
            "is_active": s.is_active == 1,
            "created_at": s.created_at,
            "updated_at": s.updated_at,
            "total_tokens": s.total_tokens,
            "total_cost": s.total_cost,
            "message_count": s.message_count,
        }))),
        None => Err(AppError::NotFound(format!(
            "Session {session_id} not found"
        ))),
    }
}

pub async fn stop_completion(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Json<serde_json::Value> {
    state.claude_manager.stop_session(&session_id).await;
    tracing::info!(session_id = %session_id, "Chat completion stopped");
    Json(json!({
        "session_id": session_id,
        "status": "stopped",
    }))
}
