use serde_json::Value;

/// Extract text content from a Claude JSONL assistant message.
///
/// Claude messages have `message.content` which can be:
/// - A string
/// - An array of content blocks: `[{"type":"text","text":"..."},...]`
pub fn extract_assistant_content(msg: &Value) -> Option<String> {
    let message = msg.get("message")?;
    let content = message.get("content")?;

    match content {
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Value::Array(arr) => {
            let parts: Vec<&str> = arr
                .iter()
                .filter_map(|item| {
                    if item.get("type")?.as_str()? == "text" {
                        item.get("text")?.as_str()
                    } else {
                        None
                    }
                })
                .collect();
            let joined = parts.join("\n");
            if joined.trim().is_empty() {
                None
            } else {
                Some(joined)
            }
        }
        _ => None,
    }
}

/// Check if the message is an assistant message with content.
pub fn is_assistant_message(msg: &Value) -> bool {
    msg.get("type").and_then(|v| v.as_str()) == Some("assistant")
        && msg
            .get("message")
            .and_then(|m| m.get("content"))
            .is_some()
}

/// Check if this is a final result message.
pub fn is_result_message(msg: &Value) -> bool {
    msg.get("type").and_then(|v| v.as_str()) == Some("result")
}

/// Usage information extracted from a Claude message.
pub struct UsageInfo {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cost_usd: f64,
}

/// Extract usage information from a message (typically the result message).
pub fn extract_usage(msg: &Value) -> Option<UsageInfo> {
    let usage = msg.get("usage")?;
    Some(UsageInfo {
        input_tokens: usage
            .get("input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        output_tokens: usage
            .get("output_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        cost_usd: msg.get("cost_usd").and_then(|v| v.as_f64()).unwrap_or(0.0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_text_content_string() {
        let msg = json!({
            "type": "assistant",
            "message": {"role": "assistant", "content": "Hello world"}
        });
        assert_eq!(
            extract_assistant_content(&msg),
            Some("Hello world".to_string())
        );
    }

    #[test]
    fn test_extract_text_content_array() {
        let msg = json!({
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": [
                    {"type": "text", "text": "Hello"},
                    {"type": "text", "text": "World"}
                ]
            }
        });
        assert_eq!(
            extract_assistant_content(&msg),
            Some("Hello\nWorld".to_string())
        );
    }

    #[test]
    fn test_is_assistant_message() {
        let msg = json!({"type": "assistant", "message": {"content": "hi"}});
        assert!(is_assistant_message(&msg));

        let msg = json!({"type": "user", "message": {"content": "hi"}});
        assert!(!is_assistant_message(&msg));
    }

    #[test]
    fn test_is_result_message() {
        let msg = json!({"type": "result"});
        assert!(is_result_message(&msg));

        let msg = json!({"type": "assistant"});
        assert!(!is_result_message(&msg));
    }

    #[test]
    fn test_extract_usage() {
        let msg = json!({
            "type": "result",
            "usage": {"input_tokens": 100, "output_tokens": 50},
            "cost_usd": 0.005
        });
        let usage = extract_usage(&msg).unwrap();
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
        assert!((usage.cost_usd - 0.005).abs() < f64::EPSILON);
    }
}
