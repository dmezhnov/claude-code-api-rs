use serde_json::json;

/// Format a JSON value as an SSE `data:` event.
pub fn sse_event(data: &serde_json::Value) -> String {
    format!(
        "data: {}\n\n",
        serde_json::to_string(data).unwrap_or_default()
    )
}

/// The SSE completion signal.
pub fn sse_done() -> String {
    "data: [DONE]\n\n".to_string()
}

/// Initial streaming chunk (role=assistant, empty content).
pub fn initial_chunk(id: &str, model: &str, created: i64) -> serde_json::Value {
    json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {"role": "assistant", "content": ""},
            "finish_reason": null
        }]
    })
}

/// Content delta chunk.
pub fn content_chunk(
    id: &str,
    model: &str,
    created: i64,
    content: &str,
) -> serde_json::Value {
    json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {"content": content},
            "finish_reason": null
        }]
    })
}

/// Final chunk with finish_reason.
pub fn final_chunk(
    id: &str,
    model: &str,
    created: i64,
    finish_reason: &str,
) -> serde_json::Value {
    json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {},
            "finish_reason": finish_reason
        }]
    })
}

/// Wrap a complete `chat.completion` response as SSE events.
///
/// Used when tool_calls force non-streaming collection but the client
/// originally requested streaming.
pub fn wrap_response_as_sse(response: &serde_json::Value) -> Vec<String> {
    let mut events = Vec::new();

    let id = response
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("chatcmpl-unknown");
    let created = response.get("created").and_then(|v| v.as_i64()).unwrap_or(0);
    let model = response
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    // Initial chunk with role
    events.push(sse_event(&json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [{"index": 0, "delta": {"role": "assistant"}, "finish_reason": null}]
    })));

    let choice = response
        .get("choices")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first());

    if let Some(choice) = choice {
        let message = choice.get("message");
        let content = message
            .and_then(|m| m.get("content"))
            .and_then(|v| v.as_str());
        let tool_calls = message
            .and_then(|m| m.get("tool_calls"))
            .and_then(|v| v.as_array());
        let finish_reason = choice
            .get("finish_reason")
            .and_then(|v| v.as_str())
            .unwrap_or("stop");

        if let Some(text) = content {
            events.push(sse_event(&content_chunk(id, model, created, text)));
        }

        if let Some(tcs) = tool_calls {
            for (i, tc) in tcs.iter().enumerate() {
                events.push(sse_event(&json!({
                    "id": id,
                    "object": "chat.completion.chunk",
                    "created": created,
                    "model": model,
                    "choices": [{
                        "index": 0,
                        "delta": {"tool_calls": [{
                            "index": i,
                            "id": tc.get("id"),
                            "type": "function",
                            "function": tc.get("function"),
                        }]},
                        "finish_reason": null
                    }]
                })));
            }
        }

        events.push(sse_event(&final_chunk(id, model, created, finish_reason)));
    }

    events.push(sse_done());
    events
}
