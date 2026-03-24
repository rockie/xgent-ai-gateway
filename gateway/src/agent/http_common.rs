/// Extract a value from a JSON object using dot-notation path.
///
/// Supports nested objects (`result.text`) and array indices (`data.0.id`).
/// Returns the value as a string: strings are returned directly, numbers/booleans/null
/// are JSON-serialized, objects/arrays are compact JSON.
pub fn extract_json_value(root: &serde_json::Value, path: &str) -> Result<String, String> {
    let segments: Vec<&str> = path.split('.').collect();
    let mut current = root;

    for segment in &segments {
        if let Ok(index) = segment.parse::<usize>() {
            current = current.get(index).ok_or_else(|| {
                format!("array index {} out of bounds at path '{}'", index, path)
            })?;
        } else {
            current = current.get(*segment).ok_or_else(|| {
                format!(
                    "key '{}' not found at path '{}'; response: {}",
                    segment,
                    path,
                    serde_json::to_string(root).unwrap_or_default()
                )
            })?;
        }
    }

    match current {
        serde_json::Value::String(s) => Ok(s.clone()),
        serde_json::Value::Null => Ok("null".to_string()),
        other => Ok(serde_json::to_string(other).unwrap_or_default()),
    }
}

/// Scan a template string for `<{prefix}.XXX>` placeholders and return the paths
/// (the part after "{prefix}."). Generalizes placeholder scanning for any prefix
/// such as "response", "poll_response", "submit_response".
pub fn find_prefixed_placeholders(template: &str, prefix: &str) -> Vec<String> {
    let prefix_dot = format!("{}.", prefix);
    let mut paths = Vec::new();
    let mut chars = template.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '<' {
            let mut token = String::new();
            let mut found_close = false;
            for c2 in chars.by_ref() {
                if c2 == '>' {
                    found_close = true;
                    break;
                }
                token.push(c2);
            }
            if found_close {
                if let Some(rest) = token.strip_prefix(&prefix_dot) {
                    paths.push(rest.to_string());
                }
            }
        }
    }

    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- extract_json_value tests --

    #[test]
    fn extract_nested_string_value() {
        let json: serde_json::Value =
            serde_json::from_str(r#"{"result": {"text": "hello world"}}"#).unwrap();
        let val = extract_json_value(&json, "result.text").unwrap();
        assert_eq!(val, "hello world");
    }

    #[test]
    fn extract_array_index_value() {
        let json: serde_json::Value =
            serde_json::from_str(r#"{"data": [{"id": "first"}, {"id": "second"}]}"#).unwrap();
        let val = extract_json_value(&json, "data.0.id").unwrap();
        assert_eq!(val, "first");
    }

    #[test]
    fn extract_numeric_value_serializes() {
        let json: serde_json::Value = serde_json::from_str(r#"{"count": 42}"#).unwrap();
        let val = extract_json_value(&json, "count").unwrap();
        assert_eq!(val, "42");
    }

    #[test]
    fn extract_boolean_value_serializes() {
        let json: serde_json::Value = serde_json::from_str(r#"{"active": true}"#).unwrap();
        let val = extract_json_value(&json, "active").unwrap();
        assert_eq!(val, "true");
    }

    #[test]
    fn extract_object_value_serializes() {
        let json: serde_json::Value =
            serde_json::from_str(r#"{"nested": {"a": 1, "b": 2}}"#).unwrap();
        let val = extract_json_value(&json, "nested").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&val).unwrap();
        assert_eq!(parsed["a"], 1);
        assert_eq!(parsed["b"], 2);
    }

    #[test]
    fn extract_missing_key_returns_error() {
        let json: serde_json::Value = serde_json::from_str(r#"{"foo": "bar"}"#).unwrap();
        let err = extract_json_value(&json, "missing.key").unwrap_err();
        assert!(err.contains("missing"), "error was: {}", err);
        assert!(err.contains("missing.key"), "error was: {}", err);
    }

    #[test]
    fn extract_array_out_of_bounds() {
        let json: serde_json::Value = serde_json::from_str(r#"{"data": [1, 2]}"#).unwrap();
        let err = extract_json_value(&json, "data.5").unwrap_err();
        assert!(err.contains("5"), "error was: {}", err);
        assert!(err.contains("out of bounds"), "error was: {}", err);
    }

    // -- find_prefixed_placeholders tests --

    #[test]
    fn finds_response_placeholders() {
        let template = r#"{"status": "<response.result>", "data": "<response.output>"}"#;
        let paths = find_prefixed_placeholders(template, "response");
        assert_eq!(paths, vec!["result", "output"]);
    }

    #[test]
    fn finds_poll_response_placeholders() {
        let template = r#"<poll_response.status> <poll_response.result.text>"#;
        let paths = find_prefixed_placeholders(template, "poll_response");
        assert_eq!(paths, vec!["status", "result.text"]);
    }

    #[test]
    fn finds_submit_response_placeholders() {
        let template = r#"<submit_response.job_id>"#;
        let paths = find_prefixed_placeholders(template, "submit_response");
        assert_eq!(paths, vec!["job_id"]);
    }

    #[test]
    fn ignores_other_prefixes() {
        let template = r#"<payload> <stdout> <response.data>"#;
        let paths = find_prefixed_placeholders(template, "response");
        assert_eq!(paths, vec!["data"]);
    }

    #[test]
    fn empty_template_returns_empty() {
        let paths = find_prefixed_placeholders("", "response");
        assert!(paths.is_empty());
    }

    #[test]
    fn no_matching_prefix_returns_empty() {
        let template = r#"<stdout> <stderr>"#;
        let paths = find_prefixed_placeholders(template, "response");
        assert!(paths.is_empty());
    }
}
