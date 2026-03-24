use std::time::Duration;

use async_trait::async_trait;
use reqwest::redirect::Policy;
use tracing::warn;

use super::config::{ResponseSection, SyncApiSection};
use super::executor::{ExecutionResult, Executor};
use super::placeholder;
use super::response;
use xgent_proto::TaskAssignment;

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

/// Scan a template string for `<response.XXX>` placeholders and return the paths
/// (the part after "response.").
fn find_response_placeholders(template: &str) -> Vec<String> {
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
                if let Some(rest) = token.strip_prefix("response.") {
                    paths.push(rest.to_string());
                }
            }
        }
    }

    paths
}

/// HTTP dispatch executor for sync-api mode.
///
/// Sends HTTP requests to a configurable endpoint with placeholder-resolved
/// URL, body, and headers. Extracts JSON response values via dot-notation
/// and maps them into the response body template.
pub struct SyncApiExecutor {
    service_name: String,
    sync_api: SyncApiSection,
    response: ResponseSection,
    client: reqwest::Client,
}

impl SyncApiExecutor {
    /// Create a new SyncApiExecutor with the given configuration.
    ///
    /// Builds a reqwest::Client with the configured timeout, redirect policy,
    /// and optional TLS certificate verification skip.
    pub fn new(
        service_name: String,
        sync_api: SyncApiSection,
        response: ResponseSection,
    ) -> Result<Self, String> {
        let mut builder = reqwest::Client::builder()
            .timeout(Duration::from_secs(sync_api.timeout_secs))
            .redirect(Policy::limited(5));

        if sync_api.tls_skip_verify {
            builder = builder.danger_accept_invalid_certs(true);
        }

        let client = builder
            .build()
            .map_err(|e| format!("failed to build HTTP client: {}", e))?;

        Ok(Self {
            service_name,
            sync_api,
            response,
            client,
        })
    }

    /// Send the HTTP request, retrying once on connection errors.
    async fn send_request(
        &self,
        method: &reqwest::Method,
        url: &str,
        headers: &reqwest::header::HeaderMap,
        body: Option<String>,
    ) -> Result<reqwest::Response, ExecutionResult> {
        let build_request = || {
            let mut req = self.client.request(method.clone(), url);
            req = req.headers(headers.clone());
            if let Some(ref b) = body {
                req = req.body(b.clone());
            }
            req
        };

        match build_request().send().await {
            Ok(resp) => Ok(resp),
            Err(e) => {
                if e.is_timeout() {
                    return Err(ExecutionResult {
                        success: false,
                        result: Vec::new(),
                        error_message: format!(
                            "HTTP request timed out after {}s",
                            self.sync_api.timeout_secs
                        ),
                    });
                }

                if e.is_connect() {
                    warn!(
                        url = url,
                        error = %e,
                        "HTTP connection failed, retrying once"
                    );

                    // Retry once on connection failure
                    match build_request().send().await {
                        Ok(resp) => Ok(resp),
                        Err(retry_err) => {
                            if retry_err.is_timeout() {
                                Err(ExecutionResult {
                                    success: false,
                                    result: Vec::new(),
                                    error_message: format!(
                                        "HTTP request timed out after {}s",
                                        self.sync_api.timeout_secs
                                    ),
                                })
                            } else {
                                Err(ExecutionResult {
                                    success: false,
                                    result: Vec::new(),
                                    error_message: format!(
                                        "HTTP request failed after retry: {}",
                                        retry_err
                                    ),
                                })
                            }
                        }
                    }
                } else {
                    Err(ExecutionResult {
                        success: false,
                        result: Vec::new(),
                        error_message: format!("HTTP request failed: {}", e),
                    })
                }
            }
        }
    }
}

#[async_trait]
impl Executor for SyncApiExecutor {
    async fn execute(&self, assignment: &TaskAssignment) -> ExecutionResult {
        // (a) Build task variables
        let mut variables = placeholder::build_task_variables(assignment, &self.service_name);

        // (b) Resolve URL placeholders
        let resolved_url = match placeholder::resolve_placeholders(&self.sync_api.url, &variables) {
            Ok(u) => u,
            Err(e) => {
                return ExecutionResult {
                    success: false,
                    result: Vec::new(),
                    error_message: format!("failed to resolve URL placeholder: {}", e),
                };
            }
        };

        // (c) Resolve body template if present
        let resolved_body = match &self.sync_api.body {
            Some(body_template) => {
                match placeholder::resolve_placeholders(body_template, &variables) {
                    Ok(b) => Some(b),
                    Err(e) => {
                        return ExecutionResult {
                            success: false,
                            result: Vec::new(),
                            error_message: format!("failed to resolve body placeholder: {}", e),
                        };
                    }
                }
            }
            None => None,
        };

        // (d) Resolve header values
        let mut header_map = reqwest::header::HeaderMap::new();
        for (key, value_template) in &self.sync_api.headers {
            let resolved_value =
                match placeholder::resolve_placeholders(value_template, &variables) {
                    Ok(v) => v,
                    Err(e) => {
                        return ExecutionResult {
                            success: false,
                            result: Vec::new(),
                            error_message: format!(
                                "failed to resolve header '{}' placeholder: {}",
                                key, e
                            ),
                        };
                    }
                };

            let header_name = match reqwest::header::HeaderName::from_bytes(key.as_bytes()) {
                Ok(n) => n,
                Err(e) => {
                    return ExecutionResult {
                        success: false,
                        result: Vec::new(),
                        error_message: format!("invalid header name '{}': {}", key, e),
                    };
                }
            };

            let header_value = match reqwest::header::HeaderValue::from_str(&resolved_value) {
                Ok(v) => v,
                Err(e) => {
                    return ExecutionResult {
                        success: false,
                        result: Vec::new(),
                        error_message: format!("invalid header value for '{}': {}", key, e),
                    };
                }
            };

            header_map.insert(header_name, header_value);
        }

        // (e) Parse method
        let method = reqwest::Method::from_bytes(self.sync_api.method.to_uppercase().as_bytes())
            .unwrap_or(reqwest::Method::POST);

        // (f) Send HTTP request with retry logic
        let resp = match self
            .send_request(&method, &resolved_url, &header_map, resolved_body)
            .await
        {
            Ok(r) => r,
            Err(exec_result) => return exec_result,
        };

        // (g) Read response status and body
        let status = resp.status();
        let body_text = match resp.text().await {
            Ok(t) => t,
            Err(e) => {
                return ExecutionResult {
                    success: false,
                    result: Vec::new(),
                    error_message: format!("failed to read response body: {}", e),
                };
            }
        };

        // (h) Check HTTP status
        if !status.is_success() {
            return ExecutionResult {
                success: false,
                result: Vec::new(),
                error_message: format!("HTTP {}: {}", status.as_u16(), body_text),
            };
        }

        // (i) Check body size against max_bytes
        if body_text.len() > self.response.max_bytes {
            return ExecutionResult {
                success: false,
                result: Vec::new(),
                error_message: format!(
                    "response body size {} bytes exceeds limit of {} bytes",
                    body_text.len(),
                    self.response.max_bytes
                ),
            };
        }

        // (j) Parse body as JSON
        let json_value: serde_json::Value = match serde_json::from_str(&body_text) {
            Ok(v) => v,
            Err(e) => {
                return ExecutionResult {
                    success: false,
                    result: Vec::new(),
                    error_message: format!("failed to parse response JSON: {}", e),
                };
            }
        };

        // (k) Scan response template for <response.XXX> placeholders and extract values
        let response_paths = find_response_placeholders(&self.response.body);
        for path in &response_paths {
            match extract_json_value(&json_value, path) {
                Ok(value) => {
                    variables.insert(format!("response.{}", path), value);
                }
                Err(e) => {
                    return ExecutionResult {
                        success: false,
                        result: Vec::new(),
                        error_message: format!("failed to extract response.{}: {}", path, e),
                    };
                }
            }
        }

        // (l) Resolve final response body template
        match response::resolve_response_body(&self.response.body, &variables, self.response.max_bytes)
        {
            Ok(bytes) => ExecutionResult {
                success: true,
                result: bytes,
                error_message: String::new(),
            },
            Err(e) => ExecutionResult {
                success: false,
                result: Vec::new(),
                error_message: e,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::any, Router};
    use std::collections::HashMap;
    use tokio::net::TcpListener;

    /// Spin up a tiny axum server that returns the given status and body for any request.
    /// Returns the base URL (e.g., "http://127.0.0.1:PORT").
    async fn start_test_server(
        status: axum::http::StatusCode,
        body: &'static str,
    ) -> String {
        let app = Router::new().route(
            "/{*path}",
            any(move || async move { (status, body.to_string()) }),
        );
        // Also handle root path
        let app = app.route(
            "/",
            any(move || async move { (status, body.to_string()) }),
        );

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        format!("http://127.0.0.1:{}", addr.port())
    }

    /// Start a test server that echoes request info back as JSON.
    async fn start_echo_server() -> String {
        let app = Router::new().route(
            "/{*path}",
            any(
                |method: axum::http::Method,
                 headers: axum::http::HeaderMap,
                 body: String| async move {
                    let mut hdr_map = serde_json::Map::new();
                    for (name, value) in &headers {
                        if let Ok(v) = value.to_str() {
                            hdr_map.insert(
                                name.as_str().to_string(),
                                serde_json::Value::String(v.to_string()),
                            );
                        }
                    }
                    let echo = serde_json::json!({
                        "method": method.as_str(),
                        "headers": hdr_map,
                        "body": body,
                    });
                    axum::Json(echo)
                },
            ),
        );

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        format!("http://127.0.0.1:{}", addr.port())
    }

    fn make_sync_api(url: &str, method: &str, body: Option<&str>) -> SyncApiSection {
        SyncApiSection {
            url: url.to_string(),
            method: method.to_string(),
            headers: HashMap::new(),
            body: body.map(|s| s.to_string()),
            timeout_secs: 10,
            tls_skip_verify: false,
        }
    }

    fn make_response(body: &str) -> ResponseSection {
        ResponseSection {
            body: body.to_string(),
            max_bytes: 1_048_576,
        }
    }

    fn make_assignment(payload: &[u8]) -> TaskAssignment {
        TaskAssignment {
            task_id: "test-task-1".to_string(),
            payload: payload.to_vec(),
            metadata: HashMap::new(),
        }
    }

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
        // Should be compact JSON
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

    // -- SyncApiExecutor integration tests --

    #[tokio::test]
    async fn sends_post_with_body_template() {
        let base_url = start_test_server(
            axum::http::StatusCode::OK,
            r#"{"result": "ok", "output": "processed"}"#,
        )
        .await;

        let sync_api = make_sync_api(
            &format!("{}/api/run", base_url),
            "POST",
            Some(r#"{"input": "<payload>"}"#),
        );
        let response = make_response(r#"{"status": "<response.result>", "data": "<response.output>"}"#);
        let executor =
            SyncApiExecutor::new("test-svc".to_string(), sync_api, response).unwrap();
        let assignment = make_assignment(b"hello");

        let result = executor.execute(&assignment).await;
        assert!(result.success, "error: {}", result.error_message);
        let output = String::from_utf8_lossy(&result.result);
        assert!(output.contains("ok"), "output was: {}", output);
        assert!(output.contains("processed"), "output was: {}", output);
    }

    #[tokio::test]
    async fn sends_get_without_body() {
        let base_url = start_test_server(
            axum::http::StatusCode::OK,
            r#"{"status": "healthy"}"#,
        )
        .await;

        let sync_api = make_sync_api(&format!("{}/health", base_url), "GET", None);
        let response = make_response("<response.status>");
        let executor =
            SyncApiExecutor::new("test-svc".to_string(), sync_api, response).unwrap();
        let assignment = make_assignment(b"");

        let result = executor.execute(&assignment).await;
        assert!(result.success, "error: {}", result.error_message);
        assert_eq!(String::from_utf8_lossy(&result.result), "healthy");
    }

    #[tokio::test]
    async fn non_2xx_returns_failure() {
        let base_url = start_test_server(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "validation error: field missing",
        )
        .await;

        let sync_api = make_sync_api(&format!("{}/api", base_url), "POST", Some("{}"));
        let response = make_response("<response.output>");
        let executor =
            SyncApiExecutor::new("test-svc".to_string(), sync_api, response).unwrap();
        let assignment = make_assignment(b"data");

        let result = executor.execute(&assignment).await;
        assert!(!result.success);
        assert!(
            result.error_message.contains("HTTP 422"),
            "error was: {}",
            result.error_message
        );
        assert!(
            result.error_message.contains("validation error"),
            "error was: {}",
            result.error_message
        );
    }

    #[tokio::test]
    async fn headers_resolve_placeholders() {
        let base_url = start_echo_server().await;

        let mut sync_api = make_sync_api(&format!("{}/api", base_url), "POST", Some("body"));
        sync_api.headers.insert(
            "X-Service".to_string(),
            "<service_name>".to_string(),
        );
        let response = make_response("<response.headers.x-service>");
        let executor =
            SyncApiExecutor::new("my-cool-svc".to_string(), sync_api, response).unwrap();
        let assignment = make_assignment(b"data");

        let result = executor.execute(&assignment).await;
        assert!(result.success, "error: {}", result.error_message);
        assert_eq!(String::from_utf8_lossy(&result.result), "my-cool-svc");
    }

    #[tokio::test]
    async fn response_template_maps_json_paths() {
        let base_url = start_test_server(
            axum::http::StatusCode::OK,
            r#"{"result": {"text": "deep value", "code": 200}}"#,
        )
        .await;

        let sync_api = make_sync_api(&format!("{}/api", base_url), "POST", Some("{}"));
        let response = make_response(r#"text=<response.result.text> code=<response.result.code>"#);
        let executor =
            SyncApiExecutor::new("test-svc".to_string(), sync_api, response).unwrap();
        let assignment = make_assignment(b"data");

        let result = executor.execute(&assignment).await;
        assert!(result.success, "error: {}", result.error_message);
        let output = String::from_utf8_lossy(&result.result);
        assert_eq!(output, "text=deep value code=200");
    }

    #[tokio::test]
    async fn url_resolves_task_placeholders() {
        let base_url = start_test_server(
            axum::http::StatusCode::OK,
            r#"{"ok": true}"#,
        )
        .await;

        let sync_api = make_sync_api(
            &format!("{}/api/<service_name>/run", base_url),
            "POST",
            Some("{}"),
        );
        let response = make_response("<response.ok>");
        let executor =
            SyncApiExecutor::new("my-svc".to_string(), sync_api, response).unwrap();
        let assignment = make_assignment(b"data");

        let result = executor.execute(&assignment).await;
        assert!(result.success, "error: {}", result.error_message);
        assert_eq!(String::from_utf8_lossy(&result.result), "true");
    }

    #[tokio::test]
    async fn large_response_exceeding_max_bytes_fails() {
        // Create a response body that exceeds max_bytes
        let large_body = format!(r#"{{"data": "{}"}}"#, "x".repeat(2000));
        let large_body_static: &'static str = Box::leak(large_body.into_boxed_str());

        let base_url = start_test_server(axum::http::StatusCode::OK, large_body_static).await;

        let sync_api = make_sync_api(&format!("{}/api", base_url), "POST", Some("{}"));
        let response = ResponseSection {
            body: "<response.data>".to_string(),
            max_bytes: 100, // Very small limit
        };
        let executor =
            SyncApiExecutor::new("test-svc".to_string(), sync_api, response).unwrap();
        let assignment = make_assignment(b"data");

        let result = executor.execute(&assignment).await;
        assert!(!result.success);
        assert!(
            result.error_message.contains("exceeds limit"),
            "error was: {}",
            result.error_message
        );
    }
}
