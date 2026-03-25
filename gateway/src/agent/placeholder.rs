use std::collections::HashMap;
use xgent_proto::TaskAssignment;

/// Single-pass placeholder resolution engine.
///
/// Scans the template for `<token>` patterns and replaces them with values
/// from the variables map. Resolved values are pushed to the output buffer
/// and never re-scanned, preventing injection from untrusted data (D-09).
///
/// Returns an error if a placeholder token is not found in the variables map,
/// listing the unresolved token and available keys.
pub fn resolve_placeholders(
    template: &str,
    variables: &HashMap<String, String>,
) -> Result<String, String> {
    let mut result = String::with_capacity(template.len());
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
                match variables.get(&token) {
                    Some(value) => result.push_str(value),
                    None => {
                        let mut available: Vec<&str> =
                            variables.keys().map(|k| k.as_str()).collect();
                        available.sort();
                        return Err(format!(
                            "unresolved placeholder <{}>; available: [{}]",
                            token,
                            available.join(", ")
                        ));
                    }
                }
            } else {
                // No closing '>' found -- preserve literal '<' and scanned chars
                result.push('<');
                result.push_str(&token);
            }
        } else {
            result.push(c);
        }
    }

    Ok(result)
}

/// Build the task variable map from a TaskAssignment and service name.
///
/// Inserts:
/// - `payload` -> the task payload (JSON string)
/// - `payload.{key}` -> for each top-level field in the payload JSON (recursively flattened)
/// - `service_name` -> the service name
/// - `metadata.{key}` -> for each entry in assignment metadata
pub fn build_task_variables(
    assignment: &TaskAssignment,
    service_name: &str,
) -> HashMap<String, String> {
    let mut vars = HashMap::new();
    vars.insert("payload".to_string(), assignment.payload.clone());
    vars.insert("service_name".to_string(), service_name.to_string());

    // Expand payload JSON fields so templates can use <payload.field> syntax
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&assignment.payload) {
        flatten_json("payload", &json, &mut vars);
    }

    for (key, value) in &assignment.metadata {
        vars.insert(format!("metadata.{}", key), value.clone());
    }

    vars
}

/// Recursively flatten a JSON value into dotted-path string entries.
/// Objects are expanded (e.g. `prefix.key`), arrays and scalars become JSON strings.
fn flatten_json(prefix: &str, value: &serde_json::Value, out: &mut HashMap<String, String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, val) in map {
                let path = format!("{}.{}", prefix, key);
                match val {
                    serde_json::Value::String(s) => {
                        // JSON-encode strings so they're valid when interpolated into JSON bodies
                        out.insert(path.clone(), serde_json::to_string(val).unwrap_or_default());
                    }
                    serde_json::Value::Object(_) => {
                        // Recurse into nested objects
                        flatten_json(&path, val, out);
                        // Also insert the serialized form for the whole object
                        out.insert(path, serde_json::to_string(val).unwrap_or_default());
                    }
                    _ => {
                        // Numbers, bools, arrays, null → JSON representation
                        out.insert(path, serde_json::to_string(val).unwrap_or_default());
                    }
                }
            }
        }
        _ => {
            // Top-level non-object: just store the JSON representation
            out.insert(prefix.to_string(), serde_json::to_string(value).unwrap_or_default());
        }
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
    fn payload_placeholder_replaced() {
        let vars = make_vars(&[("payload", "hello world")]);
        let result = resolve_placeholders("<payload>", &vars).unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn service_name_placeholder_replaced() {
        let vars = make_vars(&[("service_name", "my-svc")]);
        let result = resolve_placeholders("svc=<service_name>", &vars).unwrap();
        assert_eq!(result, "svc=my-svc");
    }

    #[test]
    fn metadata_key_replaced() {
        let vars = make_vars(&[("metadata.region", "us-east-1")]);
        let result = resolve_placeholders("region:<metadata.region>", &vars).unwrap();
        assert_eq!(result, "region:us-east-1");
    }

    #[test]
    fn metadata_missing_returns_error_with_available_keys() {
        let vars = make_vars(&[("payload", "data"), ("service_name", "svc")]);
        let err = resolve_placeholders("<metadata.missing>", &vars).unwrap_err();
        assert!(
            err.contains("unresolved placeholder <metadata.missing>"),
            "error was: {}",
            err
        );
        assert!(err.contains("payload"), "error was: {}", err);
        assert!(err.contains("service_name"), "error was: {}", err);
    }

    #[test]
    fn template_with_no_placeholders_unchanged() {
        let vars = make_vars(&[("payload", "data")]);
        let result = resolve_placeholders("no placeholders here", &vars).unwrap();
        assert_eq!(result, "no placeholders here");
    }

    #[test]
    fn single_pass_injection_safety() {
        // If payload contains "<stdout>", the resolved output should NOT re-resolve it
        let vars = make_vars(&[("payload", "<stdout>"), ("stdout", "SHOULD_NOT_APPEAR")]);
        let result = resolve_placeholders("result=<payload>", &vars).unwrap();
        assert_eq!(result, "result=<stdout>");
    }

    #[test]
    fn multiple_placeholders_in_same_template() {
        let vars = make_vars(&[
            ("payload", "data"),
            ("service_name", "svc"),
            ("stdout", "output"),
        ]);
        let result =
            resolve_placeholders("<payload>|<service_name>|<stdout>", &vars).unwrap();
        assert_eq!(result, "data|svc|output");
    }

    #[test]
    fn unclosed_angle_bracket_preserved_as_literal() {
        let vars = make_vars(&[("payload", "data")]);
        let result = resolve_placeholders("<payload without close", &vars).unwrap();
        assert_eq!(result, "<payload without close");
    }

    #[test]
    fn build_task_variables_populates_correctly() {
        let assignment = TaskAssignment {
            task_id: "task-1".to_string(),
            payload: "test payload".to_string(),
            metadata: {
                let mut m = HashMap::new();
                m.insert("region".to_string(), "us-east-1".to_string());
                m.insert("priority".to_string(), "high".to_string());
                m
            },
        };

        let vars = build_task_variables(&assignment, "my-service");
        assert_eq!(vars.get("payload").unwrap(), "test payload");
        assert_eq!(vars.get("service_name").unwrap(), "my-service");
        assert_eq!(vars.get("metadata.region").unwrap(), "us-east-1");
        assert_eq!(vars.get("metadata.priority").unwrap(), "high");
    }
}
