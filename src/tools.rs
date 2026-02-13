use regex::Regex;
use serde_json::json;
use std::sync::LazyLock;

use crate::models::openai::{FunctionCall, Tool, ToolCall};

/// Convert OpenAI tool definitions into a system prompt appendix.
pub fn format_tools_prompt(tools: &[Tool]) -> String {
    if tools.is_empty() {
        return String::new();
    }

    let mut descriptions = Vec::new();
    for tool in tools {
        let f = &tool.function;
        let mut desc = format!("- **{}**", f.name);
        if let Some(ref d) = f.description {
            desc.push_str(&format!(": {d}"));
        }
        if let Some(ref params) = f.parameters {
            if let Some(props) = params.get("properties").and_then(|v| v.as_object()) {
                let required: Vec<&str> = params
                    .get("required")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
                    .unwrap_or_default();

                let mut param_lines = Vec::new();
                for (pname, pinfo) in props {
                    let ptype = pinfo
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("any");
                    let req = if required.contains(&pname.as_str()) {
                        " (required)"
                    } else {
                        ""
                    };
                    let pdesc = pinfo
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    param_lines.push(format!("  - `{pname}` ({ptype}{req}): {pdesc}"));
                }
                if !param_lines.is_empty() {
                    desc.push('\n');
                    desc.push_str(&param_lines.join("\n"));
                }
            }
        }
        descriptions.push(desc);
    }

    format!(
        "# Available Tools\n\n\
         You have access to the following tools. To call a tool, output EXACTLY \
         this format (use a fenced code block with the language tag `tool_call`):\n\n\
         ```tool_call\n\
         {{\"name\": \"tool_name\", \"arguments\": {{\"param\": \"value\"}}}}\n\
         ```\n\n\
         Rules:\n\
         - You may include text before and/or after tool calls.\n\
         - You may call multiple tools in one response (use separate blocks).\n\
         - The arguments value must be a JSON object matching the tool's parameters.\n\
         - ALWAYS use this exact format when you want to perform an action.\n\n\
         Tools:\n\n{}",
        descriptions.join("\n\n")
    )
}

static TOOL_CALL_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)```tool_call\s*\n(.*?)\n```").unwrap());

/// Parse `tool_call` fenced blocks from Claude's response text.
///
/// Returns `(Some(tool_calls), cleaned_text)` when blocks are found,
/// or `(None, original_text)` when no blocks are present.
pub fn parse_tool_calls(text: &str) -> (Option<Vec<ToolCall>>, String) {
    let matches: Vec<_> = TOOL_CALL_PATTERN.captures_iter(text).collect();
    if matches.is_empty() {
        return (None, text.to_string());
    }

    let mut tool_calls = Vec::new();
    for cap in &matches {
        let raw = cap[1].trim();
        let data: serde_json::Value = match serde_json::from_str(raw) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let name = match data.get("name").and_then(|v| v.as_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        let arguments = data.get("arguments").cloned().unwrap_or(json!({}));
        let args_str = if arguments.is_object() {
            serde_json::to_string(&arguments).unwrap_or_default()
        } else {
            arguments.to_string()
        };

        tool_calls.push(ToolCall {
            id: generate_tool_call_id(),
            call_type: "function".to_string(),
            function: FunctionCall {
                name,
                arguments: args_str,
            },
        });
    }

    if tool_calls.is_empty() {
        return (None, text.to_string());
    }

    let cleaned = TOOL_CALL_PATTERN
        .replace_all(text, "")
        .trim()
        .to_string();
    (Some(tool_calls), cleaned)
}

fn generate_tool_call_id() -> String {
    format!("call_{}", uuid::Uuid::new_v4().as_simple())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::openai::ToolFunction;

    #[test]
    fn test_parse_single_tool_call() {
        let text = "Some text\n```tool_call\n{\"name\": \"get_weather\", \"arguments\": {\"city\": \"Paris\"}}\n```\nMore text";
        let (calls, cleaned) = parse_tool_calls(text);
        assert!(calls.is_some());
        let calls = calls.unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].function.name, "get_weather");
        assert!(calls[0].id.starts_with("call_"));
        assert!(!cleaned.contains("tool_call"));
        assert!(cleaned.contains("Some text"));
    }

    #[test]
    fn test_parse_multiple_tool_calls() {
        let text = "```tool_call\n{\"name\": \"a\", \"arguments\": {}}\n```\ntext\n```tool_call\n{\"name\": \"b\", \"arguments\": {\"x\": 1}}\n```";
        let (calls, _cleaned) = parse_tool_calls(text);
        let calls = calls.unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].function.name, "a");
        assert_eq!(calls[1].function.name, "b");
    }

    #[test]
    fn test_parse_no_tool_calls() {
        let text = "Just regular text without any tool calls";
        let (calls, cleaned) = parse_tool_calls(text);
        assert!(calls.is_none());
        assert_eq!(cleaned, text);
    }

    #[test]
    fn test_format_tools_prompt() {
        let tools = vec![Tool {
            tool_type: "function".to_string(),
            function: ToolFunction {
                name: "get_weather".to_string(),
                description: Some("Get the weather".to_string()),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "city": {"type": "string", "description": "City name"}
                    },
                    "required": ["city"]
                })),
            },
        }];
        let prompt = format_tools_prompt(&tools);
        assert!(prompt.contains("get_weather"));
        assert!(prompt.contains("City name"));
        assert!(prompt.contains("(required)"));
    }
}
