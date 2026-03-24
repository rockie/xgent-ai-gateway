use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::redirect::Policy;
use tracing::warn;

use super::config::{ResponseSection, SyncApiSection};
use super::executor::{ExecutionResult, Executor};
use super::http_common;
use super::placeholder;
use super::response;
use xgent_proto::TaskAssignment;

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
                        headers: HashMap::new(),
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
                                    headers: HashMap::new(),
                                })
                            } else {
                                Err(ExecutionResult {
                                    success: false,
                                    result: Vec::new(),
                                    error_message: format!(
                                        "HTTP request failed after retry: {}",
                                        retry_err
                                    ),
                                    headers: HashMap::new(),
                                })
                            }
                        }
                    }
                } else {
                    Err(ExecutionResult {
                        success: false,
                        result: Vec::new(),
                        error_message: format!("HTTP request failed: {}", e),
                        headers: HashMap::new(),
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
                    headers: HashMap::new(),
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
                            headers: HashMap::new(),
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
                            headers: HashMap::new(),
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
                        headers: HashMap::new(),
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
                        headers: HashMap::new(),
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
                    headers: HashMap::new(),
                };
            }
        };

        // (h) Check HTTP status
        if !status.is_success() {
            // Try to resolve failed.body template with response.* variables from error body
            let (result_bytes, hdrs) = if let Some(ref failed) = self.response.failed {
                // Try to parse error body as JSON for response.* extraction
                let mut fail_vars = variables.clone();
                if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&body_text) {
                    let paths = http_common::find_prefixed_placeholders(&failed.body, "response");
                    for path in &paths {
                        if let Ok(value) = http_common::extract_json_value(&json_value, path) {
                            fail_vars.insert(format!("response.{}", path), value);
                        }
                    }
                }
                // Also add the raw body text and status code
                fail_vars.insert("response.status".to_string(), status.as_u16().to_string());
                fail_vars.insert("response.body".to_string(), body_text.clone());

                let bytes = response::resolve_response_body(
                    &failed.body,
                    &fail_vars,
                    self.response.max_bytes,
                )
                .unwrap_or_default();
                let h = response::parse_header_json(failed.header.as_deref())
                    .unwrap_or_default();
                (bytes, h)
            } else {
                (Vec::new(), HashMap::new())
            };

            return ExecutionResult {
                success: false,
                result: result_bytes,
                error_message: format!("HTTP {}: {}", status.as_u16(), body_text),
                headers: hdrs,
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
                headers: HashMap::new(),
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
                    headers: HashMap::new(),
                };
            }
        };

        // (k) Scan response template for <response.XXX> placeholders and extract values
        let response_paths =
            http_common::find_prefixed_placeholders(&self.response.success.body, "response");
        for path in &response_paths {
            match http_common::extract_json_value(&json_value, path) {
                Ok(value) => {
                    variables.insert(format!("response.{}", path), value);
                }
                Err(e) => {
                    return ExecutionResult {
                        success: false,
                        result: Vec::new(),
                        error_message: format!("failed to extract response.{}: {}", path, e),
                        headers: HashMap::new(),
                    };
                }
            }
        }

        // (l) Resolve final response body template
        let headers =
            response::parse_header_json(self.response.success.header.as_deref()).unwrap_or_default();

        match response::resolve_response_body(
            &self.response.success.body,
            &variables,
            self.response.max_bytes,
        ) {
            Ok(bytes) => ExecutionResult {
                success: true,
                result: bytes,
                error_message: String::new(),
                headers,
            },
            Err(e) => ExecutionResult {
                success: false,
                result: Vec::new(),
                error_message: e,
                headers: HashMap::new(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::config::{FailedResponseConfig, SuccessResponseConfig};
    use axum::{routing::any, Router};
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
            success: SuccessResponseConfig {
                body: body.to_string(),
                header: None,
            },
            failed: None,
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
            success: SuccessResponseConfig {
                body: "<response.data>".to_string(),
                header: None,
            },
            failed: None,
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

    #[tokio::test]
    async fn non_2xx_resolves_failed_body_template() {
        let base_url = start_test_server(
            axum::http::StatusCode::BAD_REQUEST,
            r#"{"error": "invalid input", "code": "EINVAL"}"#,
        )
        .await;

        let sync_api = make_sync_api(&format!("{}/api", base_url), "POST", Some("{}"));
        let response = ResponseSection {
            success: SuccessResponseConfig {
                body: "<response.output>".to_string(),
                header: None,
            },
            failed: Some(FailedResponseConfig {
                body: r#"{"err": "<response.error>", "err_code": "<response.code>"}"#.to_string(),
                header: None,
            }),
            max_bytes: 1_048_576,
        };
        let executor =
            SyncApiExecutor::new("test-svc".to_string(), sync_api, response).unwrap();
        let assignment = make_assignment(b"data");

        let result = executor.execute(&assignment).await;
        assert!(!result.success);
        assert!(
            result.error_message.contains("HTTP 400"),
            "error was: {}",
            result.error_message
        );
        let output = String::from_utf8_lossy(&result.result);
        assert!(
            output.contains("invalid input"),
            "output was: {}",
            output
        );
        assert!(
            output.contains("EINVAL"),
            "output was: {}",
            output
        );
    }
}
