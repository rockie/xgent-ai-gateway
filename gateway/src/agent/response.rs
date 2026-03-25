use std::collections::HashMap;

use super::placeholder;

/// Resolve a response body template with placeholder variables,
/// enforcing a max_bytes limit on raw stdout+stderr output size.
///
/// The max_bytes check is on RAW stdout+stderr size BEFORE template resolution
/// (per RESEARCH.md Pitfall 6). This protects against runaway process output.
///
/// Returns the resolved template as bytes, or an error if:
/// - stdout+stderr combined size exceeds max_bytes
/// - A placeholder in the template cannot be resolved
pub fn resolve_response_body(
    body_template: &str,
    variables: &HashMap<String, String>,
    max_bytes: usize,
) -> Result<String, String> {
    // Check raw output size before template resolution
    let stdout_len = variables.get("stdout").map_or(0, |s| s.len());
    let stderr_len = variables.get("stderr").map_or(0, |s| s.len());
    let total = stdout_len + stderr_len;

    if total > max_bytes {
        return Err(format!(
            "output size {} bytes exceeds limit of {} bytes",
            total, max_bytes
        ));
    }

    let resolved = placeholder::resolve_placeholders(body_template, variables)?;
    Ok(resolved)
}

/// Parse a JSON string of headers into a HashMap.
/// Returns empty HashMap if input is None.
pub fn parse_header_json(header_json: Option<&str>) -> Result<HashMap<String, String>, String> {
    match header_json {
        None => Ok(HashMap::new()),
        Some(json_str) => serde_json::from_str::<HashMap<String, String>>(json_str)
            .map_err(|e| format!("invalid header JSON '{}': {}", json_str, e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_vars(entries: &[(&str, &str)]) -> HashMap<String, String> {
        entries
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn stdout_stderr_template_resolves() {
        let vars = make_vars(&[("stdout", "hello"), ("stderr", "")]);
        let template = r#"{"output": "<stdout>", "errors": "<stderr>"}"#;
        let result = resolve_response_body(template, &vars, 1_048_576).unwrap();
        assert_eq!(result, r#"{"output": "hello", "errors": ""}"#);
    }

    #[test]
    fn payload_and_service_name_resolve() {
        let vars = make_vars(&[
            ("payload", "input-data"),
            ("service_name", "my-svc"),
            ("stdout", "out"),
            ("stderr", ""),
        ]);
        let template = r#"{"payload": "<payload>", "service": "<service_name>", "out": "<stdout>"}"#;
        let result = resolve_response_body(template, &vars, 1_048_576).unwrap();
        assert_eq!(
            result,
            r#"{"payload": "input-data", "service": "my-svc", "out": "out"}"#
        );
    }

    #[test]
    fn exceeding_max_bytes_returns_error() {
        let stdout = "a".repeat(500);
        let stderr = "b".repeat(600);
        let vars = make_vars(&[("stdout", &stdout), ("stderr", &stderr)]);
        let template = "<stdout>";
        let err = resolve_response_body(template, &vars, 1000).unwrap_err();
        assert!(err.contains("output size"), "error was: {}", err);
        assert!(err.contains("1100"), "error was: {}", err);
        assert!(err.contains("exceeds limit"), "error was: {}", err);
        assert!(err.contains("1000"), "error was: {}", err);
    }

    #[test]
    fn within_max_bytes_resolves_ok() {
        let stdout = "a".repeat(500);
        let stderr = "b".repeat(499);
        let vars = make_vars(&[("stdout", &stdout), ("stderr", &stderr)]);
        let template = "<stdout>";
        let result = resolve_response_body(template, &vars, 1000).unwrap();
        assert_eq!(result.len(), 500);
    }

    #[test]
    fn parse_header_json_valid() {
        let result =
            super::parse_header_json(Some(r#"{"Content-Type": "application/json"}"#)).unwrap();
        assert_eq!(result.get("Content-Type").unwrap(), "application/json");
    }

    #[test]
    fn parse_header_json_none_returns_empty() {
        let result = super::parse_header_json(None).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn parse_header_json_invalid_returns_error() {
        let err = super::parse_header_json(Some("not json")).unwrap_err();
        assert!(err.contains("invalid header JSON"), "error was: {}", err);
    }

    #[test]
    fn metadata_key_resolves_in_body() {
        let vars = make_vars(&[
            ("stdout", "ok"),
            ("stderr", ""),
            ("metadata.region", "us-east-1"),
        ]);
        let template = r#"{"region": "<metadata.region>", "out": "<stdout>"}"#;
        let result = resolve_response_body(template, &vars, 1_048_576).unwrap();
        assert_eq!(result, r#"{"region": "us-east-1", "out": "ok"}"#);
    }
}
