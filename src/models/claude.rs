/// Model alias mappings: short name -> actual Claude CLI model name.
const MODEL_ALIASES: &[(&str, &str)] = &[
    ("cc-sonnet-45", "claude-sonnet-4-5-20250929"),
    ("cc-haiku-45", "claude-haiku-4-5-20251001"),
];

/// Resolve model aliases and validate the model name.
/// Returns the actual Claude CLI model string.
pub fn validate_claude_model(model: &str) -> String {
    // Check aliases first
    for (alias, actual) in MODEL_ALIASES {
        if model == *alias {
            return actual.to_string();
        }
    }

    // Pass through any model starting with "claude-"
    if model.starts_with("claude-") {
        return model.to_string();
    }

    // Fallback to Sonnet 4.5 for unknown models
    tracing::warn!(model, "Unknown model, falling back to claude-sonnet-4-5-20250929");
    "claude-sonnet-4-5-20250929".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alias_resolution() {
        assert_eq!(validate_claude_model("cc-sonnet-45"), "claude-sonnet-4-5-20250929");
        assert_eq!(validate_claude_model("cc-haiku-45"), "claude-haiku-4-5-20251001");
    }

    #[test]
    fn test_passthrough() {
        assert_eq!(validate_claude_model("claude-opus-4-6"), "claude-opus-4-6");
        assert_eq!(
            validate_claude_model("claude-3-7-sonnet-20250219"),
            "claude-3-7-sonnet-20250219"
        );
    }

    #[test]
    fn test_fallback() {
        assert_eq!(validate_claude_model("gpt-4"), "claude-sonnet-4-5-20250929");
    }
}
