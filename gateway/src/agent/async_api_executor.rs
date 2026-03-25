use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::redirect::Policy;
use tracing::{info, warn};

use super::config::{AsyncApiSection, ResponseSection};
use super::executor::{ExecutionResult, Executor};
use super::http_common;
use super::placeholder;
use super::response;
use xgent_proto::TaskAssignment;

/// HTTP dispatch executor for async-api mode.
///
/// Implements a two-phase submit+poll pattern: submits a job via HTTP,
/// then polls at a configurable interval until a completion or failure
/// condition is detected. The entire flow is wrapped in a timeout.
pub struct AsyncApiExecutor {
    service_name: String,
    async_api: AsyncApiSection,
    response: ResponseSection,
    client: reqwest::Client,
    dump_request_body: bool,
    dump_submit_response: bool,
    dump_poll_response: bool,
}

impl AsyncApiExecutor {
    /// Create a new AsyncApiExecutor with the given configuration.
    ///
    /// Builds a reqwest::Client WITHOUT per-request timeout -- the outer
    /// tokio::time::timeout handles total duration (D-08).
    pub fn new(
        service_name: String,
        async_api: AsyncApiSection,
        response: ResponseSection,
        dump_request_body: bool,
        dump_submit_response: bool,
        dump_poll_response: bool,
    ) -> Result<Self, String> {
        let mut builder = reqwest::Client::builder().redirect(Policy::limited(5));
        if async_api.tls_skip_verify {
            builder = builder.danger_accept_invalid_certs(true);
        }
        let client = builder
            .build()
            .map_err(|e| format!("failed to build HTTP client: {}", e))?;
        Ok(Self {
            service_name,
            async_api,
            response,
            client,
            dump_request_body,
            dump_submit_response,
            dump_poll_response,
        })
    }

    /// Send an HTTP request, retrying once on connection errors (D-13/D-14).
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
                if e.is_connect() {
                    warn!(url = url, error = %e, "HTTP connection failed, retrying once");
                    match build_request().send().await {
                        Ok(resp) => Ok(resp),
                        Err(retry_err) => Err(ExecutionResult {
                            success: false,
                            result: String::new(),
                            error_message: format!(
                                "HTTP request failed after retry: {}",
                                retry_err
                            ),
                            headers: HashMap::new(),
                        }),
                    }
                } else {
                    Err(ExecutionResult {
                        success: false,
                        result: String::new(),
                        error_message: format!("HTTP request failed: {}", e),
                        headers: HashMap::new(),
                    })
                }
            }
        }
    }

    /// Resolve header templates and build a reqwest HeaderMap.
    fn resolve_headers(
        config_headers: &HashMap<String, String>,
        variables: &HashMap<String, String>,
    ) -> Result<reqwest::header::HeaderMap, ExecutionResult> {
        let mut header_map = reqwest::header::HeaderMap::new();
        for (key, value_template) in config_headers {
            let resolved_value =
                match placeholder::resolve_placeholders(value_template, variables) {
                    Ok(v) => v,
                    Err(e) => {
                        return Err(ExecutionResult {
                            success: false,
                            result: String::new(),
                            error_message: format!(
                                "failed to resolve header '{}' placeholder: {}",
                                key, e
                            ),
                            headers: HashMap::new(),
                        });
                    }
                };

            let header_name = match reqwest::header::HeaderName::from_bytes(key.as_bytes()) {
                Ok(n) => n,
                Err(e) => {
                    return Err(ExecutionResult {
                        success: false,
                        result: String::new(),
                        error_message: format!("invalid header name '{}': {}", key, e),
                        headers: HashMap::new(),
                    });
                }
            };

            let header_value = match reqwest::header::HeaderValue::from_str(&resolved_value) {
                Ok(v) => v,
                Err(e) => {
                    return Err(ExecutionResult {
                        success: false,
                        result: String::new(),
                        error_message: format!("invalid header value for '{}': {}", key, e),
                        headers: HashMap::new(),
                    });
                }
            };

            header_map.insert(header_name, header_value);
        }
        Ok(header_map)
    }

    /// Core submit+poll logic, called inside tokio::time::timeout.
    async fn run_submit_poll(&self, assignment: &TaskAssignment) -> ExecutionResult {
        // (1) Build task variables
        let mut variables = placeholder::build_task_variables(assignment, &self.service_name);

        // (2) Resolve submit URL
        let submit_url = match placeholder::resolve_placeholders(
            &self.async_api.submit.url,
            &variables,
        ) {
            Ok(u) => u,
            Err(e) => {
                return ExecutionResult {
                    success: false,
                    result: String::new(),
                    error_message: format!("failed to resolve submit URL placeholder: {}", e),
                    headers: HashMap::new(),
                };
            }
        };

        // (3) Resolve submit body
        let submit_body = match &self.async_api.submit.body {
            Some(body_template) => {
                match placeholder::resolve_placeholders(body_template, &variables) {
                    Ok(b) => Some(b),
                    Err(e) => {
                        return ExecutionResult {
                            success: false,
                            result: String::new(),
                            error_message: format!(
                                "failed to resolve submit body placeholder: {}",
                                e
                            ),
                            headers: HashMap::new(),
                        };
                    }
                }
            }
            None => None,
        };

        // (4) Resolve submit headers
        let submit_headers =
            match Self::resolve_headers(&self.async_api.submit.headers, &variables) {
                Ok(h) => h,
                Err(exec_result) => return exec_result,
            };

        // (5) Parse submit method
        let submit_method =
            reqwest::Method::from_bytes(self.async_api.submit.method.to_uppercase().as_bytes())
                .unwrap_or(reqwest::Method::POST);

        // Dump resolved request for debugging
        if self.dump_request_body {
            tracing::info!(
                url = %submit_url,
                method = %submit_method,
                body = submit_body.as_deref().unwrap_or("(none)"),
                "dump_request_body [submit]"
            );
        }

        // (6) Send submit request
        info!(url = %submit_url, method = %submit_method, "sending async-api submit request");
        let submit_resp = match self
            .send_request(&submit_method, &submit_url, &submit_headers, submit_body)
            .await
        {
            Ok(r) => r,
            Err(exec_result) => return exec_result,
        };

        // (7) Check submit response status (D-15)
        let submit_status = submit_resp.status();
        let submit_body_text = match submit_resp.text().await {
            Ok(t) => t,
            Err(e) => {
                return ExecutionResult {
                    success: false,
                    result: String::new(),
                    error_message: format!("failed to read submit response body: {}", e),
                    headers: HashMap::new(),
                };
            }
        };

        if !submit_status.is_success() {
            return ExecutionResult {
                success: false,
                result: String::new(),
                error_message: format!("submit HTTP {}: {}", submit_status.as_u16(), submit_body_text),
                headers: HashMap::new(),
            };
        }

        // (8) Parse submit response as JSON
        let submit_json: serde_json::Value = match serde_json::from_str(&submit_body_text) {
            Ok(v) => v,
            Err(e) => {
                return ExecutionResult {
                    success: false,
                    result: String::new(),
                    error_message: format!("failed to parse submit response JSON: {}", e),
                    headers: HashMap::new(),
                };
            }
        };

        if self.dump_submit_response {
            tracing::info!(body = %submit_body_text, "dump_submit_response");
        }

        // (9) Scan poll URL and poll body for <submit_response.*> placeholders
        let mut submit_paths =
            http_common::find_prefixed_placeholders(&self.async_api.poll.url, "submit_response");
        if let Some(ref poll_body) = self.async_api.poll.body {
            submit_paths.extend(http_common::find_prefixed_placeholders(
                poll_body,
                "submit_response",
            ));
        }
        // Also scan poll headers for submit_response placeholders
        for value_template in self.async_api.poll.headers.values() {
            submit_paths.extend(http_common::find_prefixed_placeholders(
                value_template,
                "submit_response",
            ));
        }

        // (10) Insert whole submit response body and extract dot-path values
        variables.insert("submit_response".to_string(), submit_body_text.clone());
        for path in &submit_paths {
            match http_common::extract_json_value(&submit_json, path) {
                Ok(value) => {
                    variables.insert(format!("submit_response.{}", path), value);
                }
                Err(e) => {
                    return ExecutionResult {
                        success: false,
                        result: String::new(),
                        error_message: format!(
                            "failed to extract submit_response.{}: {}",
                            path, e
                        ),
                        headers: HashMap::new(),
                    };
                }
            }
        }

        // (11) Poll loop
        let mut poll_iteration = 0u32;
        loop {
            // (11a) Sleep before polling
            tokio::time::sleep(Duration::from_secs(self.async_api.poll.interval_secs)).await;
            poll_iteration += 1;

            // (11b) Resolve poll URL and body
            let poll_url =
                match placeholder::resolve_placeholders(&self.async_api.poll.url, &variables) {
                    Ok(u) => u,
                    Err(e) => {
                        return ExecutionResult {
                            success: false,
                            result: String::new(),
                            error_message: format!(
                                "failed to resolve poll URL placeholder: {}",
                                e
                            ),
                            headers: HashMap::new(),
                        };
                    }
                };

            let poll_body = match &self.async_api.poll.body {
                Some(body_template) => {
                    match placeholder::resolve_placeholders(body_template, &variables) {
                        Ok(b) => Some(b),
                        Err(e) => {
                            return ExecutionResult {
                                success: false,
                                result: String::new(),
                                error_message: format!(
                                    "failed to resolve poll body placeholder: {}",
                                    e
                                ),
                                headers: HashMap::new(),
                            };
                        }
                    }
                }
                None => None,
            };

            // (11c) Resolve poll headers
            let poll_headers =
                match Self::resolve_headers(&self.async_api.poll.headers, &variables) {
                    Ok(h) => h,
                    Err(exec_result) => return exec_result,
                };

            // (11d) Parse poll method
            let poll_method = reqwest::Method::from_bytes(
                self.async_api.poll.method.to_uppercase().as_bytes(),
            )
            .unwrap_or(reqwest::Method::GET);

            // (11e) Send poll request
            info!(
                url = %poll_url,
                method = %poll_method,
                iteration = poll_iteration,
                "sending poll request"
            );
            let poll_resp = match self
                .send_request(&poll_method, &poll_url, &poll_headers, poll_body)
                .await
            {
                Ok(r) => r,
                Err(exec_result) => return exec_result,
            };

            // (11f) Check poll response status (D-15)
            let poll_status = poll_resp.status();
            let poll_body_text = match poll_resp.text().await {
                Ok(t) => t,
                Err(e) => {
                    return ExecutionResult {
                        success: false,
                        result: String::new(),
                        error_message: format!("failed to read poll response body: {}", e),
                        headers: HashMap::new(),
                    };
                }
            };

            if !poll_status.is_success() {
                return ExecutionResult {
                    success: false,
                    result: String::new(),
                    error_message: format!(
                        "poll HTTP {}: {}",
                        poll_status.as_u16(),
                        poll_body_text
                    ),
                    headers: HashMap::new(),
                };
            }

            if self.dump_poll_response {
                tracing::info!(iteration = poll_iteration, body = %poll_body_text, "dump_poll_response");
            }

            // (11g) Parse poll response as JSON (D-16)
            let poll_json: serde_json::Value = match serde_json::from_str(&poll_body_text) {
                Ok(v) => v,
                Err(e) => {
                    return ExecutionResult {
                        success: false,
                        result: String::new(),
                        error_message: format!("failed to parse poll response JSON: {}", e),
                        headers: HashMap::new(),
                    };
                }
            };

            // (11h) Check completed_when (D-10: check first)
            match self.async_api.completed_when.evaluate(&poll_json) {
                Ok(true) => {
                    info!(iteration = poll_iteration, "completion detected");

                    // Insert the whole poll response body as <poll_response>
                    variables.insert("poll_response".to_string(), poll_body_text.clone());

                    // Extract poll_response.* into variables for success body
                    let poll_paths = http_common::find_prefixed_placeholders(
                        &self.response.success.body,
                        "poll_response",
                    );
                    for path in &poll_paths {
                        match http_common::extract_json_value(&poll_json, path) {
                            Ok(value) => {
                                variables.insert(format!("poll_response.{}", path), value);
                            }
                            Err(e) => {
                                return ExecutionResult {
                                    success: false,
                                    result: String::new(),
                                    error_message: format!(
                                        "failed to extract poll_response.{}: {}",
                                        path, e
                                    ),
                                    headers: HashMap::new(),
                                };
                            }
                        }
                    }

                    // Resolve success body template
                    let headers = response::parse_header_json(
                        self.response.success.header.as_deref(),
                    )
                    .unwrap_or_default();

                    return match response::resolve_response_body(
                        &self.response.success.body,
                        &variables,
                        self.response.max_bytes,
                    ) {
                        Ok(result_str) => ExecutionResult {
                            success: true,
                            result: result_str,
                            error_message: String::new(),
                            headers,
                        },
                        Err(e) => ExecutionResult {
                            success: false,
                            result: String::new(),
                            error_message: e,
                            headers: HashMap::new(),
                        },
                    };
                }
                Ok(false) => {}
                Err(e) => {
                    return ExecutionResult {
                        success: false,
                        result: String::new(),
                        error_message: format!("completed_when evaluation error: {}", e),
                        headers: HashMap::new(),
                    };
                }
            }

            // (11i) Check failed_when (if configured)
            if let Some(ref failed_when) = self.async_api.failed_when {
                match failed_when.evaluate(&poll_json) {
                    Ok(true) => {
                        info!(iteration = poll_iteration, "failure detected");

                        // Resolve failed body template if present (D-25)
                        let (result_str, hdrs) =
                            if let Some(ref failed) = self.response.failed {
                                let mut fail_vars = variables.clone();
                                fail_vars.insert("poll_response".to_string(), poll_body_text.clone());
                                let poll_paths = http_common::find_prefixed_placeholders(
                                    &failed.body,
                                    "poll_response",
                                );
                                for path in &poll_paths {
                                    if let Ok(value) =
                                        http_common::extract_json_value(&poll_json, path)
                                    {
                                        fail_vars
                                            .insert(format!("poll_response.{}", path), value);
                                    }
                                }
                                let s = response::resolve_response_body(
                                    &failed.body,
                                    &fail_vars,
                                    self.response.max_bytes,
                                )
                                .unwrap_or_default();
                                let h = response::parse_header_json(failed.header.as_deref())
                                    .unwrap_or_default();
                                (s, h)
                            } else {
                                (String::new(), HashMap::new())
                            };

                        return ExecutionResult {
                            success: false,
                            result: result_str,
                            error_message: format!(
                                "failed_when condition matched: {} {:?} {:?}",
                                failed_when.path, failed_when.operator, failed_when.value
                            ),
                            headers: hdrs,
                        };
                    }
                    Ok(false) => {}
                    Err(e) => {
                        return ExecutionResult {
                            success: false,
                            result: String::new(),
                            error_message: format!("failed_when evaluation error: {}", e),
                            headers: HashMap::new(),
                        };
                    }
                }
            }

            // (11j) Neither completed nor failed, continue polling
            info!(iteration = poll_iteration, "poll: neither completed nor failed, continuing");
        }
    }
}

#[async_trait]
impl Executor for AsyncApiExecutor {
    async fn execute(&self, assignment: &TaskAssignment) -> ExecutionResult {
        let timeout_dur = Duration::from_secs(self.async_api.timeout_secs);
        match tokio::time::timeout(timeout_dur, self.run_submit_poll(assignment)).await {
            Ok(result) => result,
            Err(_) => ExecutionResult {
                success: false,
                result: String::new(),
                error_message: format!(
                    "async-api timed out after {}s",
                    self.async_api.timeout_secs
                ),
                headers: HashMap::new(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::config::{
        AsyncApiSection, CompletionCondition, ConditionOperator, ConditionValue,
        FailedResponseConfig, PollSection, SubmitSection, SuccessResponseConfig,
    };
    use axum::extract::State;
    use axum::routing::any;
    use axum::Router;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use tokio::net::TcpListener;

    /// Stateful test server that returns different responses based on call count.
    /// Each call increments the counter and returns the response at that index.
    /// If the counter exceeds the number of responses, the last response is reused.
    async fn start_stateful_server(
        responses: Vec<(axum::http::StatusCode, String)>,
    ) -> (String, Arc<AtomicU32>) {
        let counter = Arc::new(AtomicU32::new(0));
        let responses = Arc::new(responses);

        let app = Router::new()
            .route(
                "/{*path}",
                any(
                    |State((counter, responses)): State<(
                        Arc<AtomicU32>,
                        Arc<Vec<(axum::http::StatusCode, String)>>,
                    )>| async move {
                        let idx = counter.fetch_add(1, Ordering::SeqCst) as usize;
                        let (status, body) = if idx < responses.len() {
                            responses[idx].clone()
                        } else {
                            responses.last().unwrap().clone()
                        };
                        (status, body)
                    },
                ),
            )
            .route(
                "/",
                any(
                    |State((counter, responses)): State<(
                        Arc<AtomicU32>,
                        Arc<Vec<(axum::http::StatusCode, String)>>,
                    )>| async move {
                        let idx = counter.fetch_add(1, Ordering::SeqCst) as usize;
                        let (status, body) = if idx < responses.len() {
                            responses[idx].clone()
                        } else {
                            responses.last().unwrap().clone()
                        };
                        (status, body)
                    },
                ),
            )
            .with_state((counter.clone(), responses));

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        (format!("http://127.0.0.1:{}", addr.port()), counter)
    }

    fn make_assignment(payload: &str) -> TaskAssignment {
        TaskAssignment {
            task_id: "test-task-1".to_string(),
            payload: payload.to_string(),
            metadata: HashMap::new(),
        }
    }

    fn make_async_api(
        submit_url: &str,
        poll_url: &str,
        completed_when: CompletionCondition,
        failed_when: Option<CompletionCondition>,
    ) -> AsyncApiSection {
        AsyncApiSection {
            submit: SubmitSection {
                url: submit_url.to_string(),
                method: "POST".to_string(),
                headers: HashMap::new(),
                body: Some(r#"{"input": "<payload>"}"#.to_string()),
            },
            poll: PollSection {
                url: poll_url.to_string(),
                method: "GET".to_string(),
                headers: HashMap::new(),
                body: None,
                interval_secs: 1,
            },
            completed_when,
            failed_when,
            timeout_secs: 30,
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

    fn equal_condition(path: &str, value: &str) -> CompletionCondition {
        CompletionCondition {
            path: path.to_string(),
            operator: ConditionOperator::Equal,
            value: ConditionValue::Single(value.to_string()),
        }
    }

    fn in_condition(path: &str, values: &[&str]) -> CompletionCondition {
        CompletionCondition {
            path: path.to_string(),
            operator: ConditionOperator::In,
            value: ConditionValue::Multiple(values.iter().map(|s| s.to_string()).collect()),
        }
    }

    // -- Test 1 (AAPI-01): submit_extracts_job_id --

    #[tokio::test]
    async fn submit_extracts_job_id() {
        let (base_url, _counter) = start_stateful_server(vec![
            // Submit response
            (
                axum::http::StatusCode::OK,
                r#"{"id": "job-123", "status": "submitted"}"#.to_string(),
            ),
            // Poll response: completed immediately
            (
                axum::http::StatusCode::OK,
                r#"{"status": "completed", "result": "done"}"#.to_string(),
            ),
        ])
        .await;

        let async_api = make_async_api(
            &format!("{}/submit", base_url),
            &format!("{}/poll/<submit_response.id>", base_url),
            equal_condition("status", "completed"),
            None,
        );
        let response = make_response("<poll_response.result>");
        let executor =
            AsyncApiExecutor::new("test-svc".to_string(), async_api, response, false, false, false).unwrap();
        let assignment = make_assignment("test-input");

        let result = executor.execute(&assignment).await;
        assert!(result.success, "error: {}", result.error_message);
        assert_eq!(result.result.as_str(), "done");
    }

    // -- Test 2 (AAPI-02): poll_uses_submit_values --

    #[tokio::test]
    async fn poll_uses_submit_values() {
        let (base_url, counter) = start_stateful_server(vec![
            // Submit response
            (
                axum::http::StatusCode::OK,
                r#"{"id": "abc-456"}"#.to_string(),
            ),
            // Poll response: completed
            (
                axum::http::StatusCode::OK,
                r#"{"status": "completed", "output": "result-data"}"#.to_string(),
            ),
        ])
        .await;

        let async_api = make_async_api(
            &format!("{}/submit", base_url),
            // The poll URL includes submit_response.id placeholder
            &format!("{}/jobs/<submit_response.id>/status", base_url),
            equal_condition("status", "completed"),
            None,
        );
        let response = make_response("<poll_response.output>");
        let executor =
            AsyncApiExecutor::new("test-svc".to_string(), async_api, response, false, false, false).unwrap();
        let assignment = make_assignment("data");

        let result = executor.execute(&assignment).await;
        assert!(result.success, "error: {}", result.error_message);
        assert_eq!(result.result.as_str(), "result-data");
        // Verify at least 2 calls (submit + poll)
        assert!(counter.load(Ordering::SeqCst) >= 2);
    }

    // -- Test 3 (AAPI-03): condition_operators_complete_on_match --

    #[tokio::test]
    async fn condition_operators_complete_on_match() {
        let (base_url, _counter) = start_stateful_server(vec![
            // Submit
            (
                axum::http::StatusCode::OK,
                r#"{"id": "j1"}"#.to_string(),
            ),
            // Poll: completed
            (
                axum::http::StatusCode::OK,
                r#"{"status": "completed", "result": {"output": "success"}}"#.to_string(),
            ),
        ])
        .await;

        let async_api = make_async_api(
            &format!("{}/submit", base_url),
            &format!("{}/poll", base_url),
            equal_condition("status", "completed"),
            None,
        );
        let response = make_response(r#"{"data": "<poll_response.result.output>"}"#);
        let executor =
            AsyncApiExecutor::new("test-svc".to_string(), async_api, response, false, false, false).unwrap();
        let assignment = make_assignment("data");

        let result = executor.execute(&assignment).await;
        assert!(result.success, "error: {}", result.error_message);
        let output = result.result.as_str();
        assert!(output.contains("success"), "output was: {}", output);
    }

    // -- Test 4 (AAPI-04): failed_when_shortcircuits --

    #[tokio::test]
    async fn failed_when_shortcircuits() {
        let (base_url, counter) = start_stateful_server(vec![
            // Submit
            (
                axum::http::StatusCode::OK,
                r#"{"id": "j2"}"#.to_string(),
            ),
            // Poll: failed immediately
            (
                axum::http::StatusCode::OK,
                r#"{"status": "failed", "error": "something broke"}"#.to_string(),
            ),
        ])
        .await;

        let async_api = make_async_api(
            &format!("{}/submit", base_url),
            &format!("{}/poll", base_url),
            equal_condition("status", "completed"),
            Some(in_condition("status", &["failed", "error", "cancelled"])),
        );
        let response = make_response("<poll_response.result>");
        let executor =
            AsyncApiExecutor::new("test-svc".to_string(), async_api, response, false, false, false).unwrap();
        let assignment = make_assignment("data");

        let result = executor.execute(&assignment).await;
        assert!(!result.success);
        assert!(
            result.error_message.contains("failed_when condition matched"),
            "error was: {}",
            result.error_message
        );
        // Should only have called submit + 1 poll (no further polling)
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    // -- Test 5 (AAPI-05): timeout_cancels_polling --

    #[tokio::test]
    async fn timeout_cancels_polling() {
        let (base_url, _counter) = start_stateful_server(vec![
            // Submit
            (
                axum::http::StatusCode::OK,
                r#"{"id": "j3"}"#.to_string(),
            ),
            // Poll: always running (never completes)
            (
                axum::http::StatusCode::OK,
                r#"{"status": "running"}"#.to_string(),
            ),
        ])
        .await;

        let mut async_api = make_async_api(
            &format!("{}/submit", base_url),
            &format!("{}/poll", base_url),
            equal_condition("status", "completed"),
            None,
        );
        async_api.timeout_secs = 2; // Very short timeout
        async_api.poll.interval_secs = 1;

        let response = make_response("<poll_response.result>");
        let executor =
            AsyncApiExecutor::new("test-svc".to_string(), async_api, response, false, false, false).unwrap();
        let assignment = make_assignment("data");

        let start = std::time::Instant::now();
        let result = executor.execute(&assignment).await;
        let elapsed = start.elapsed();

        assert!(!result.success);
        assert!(
            result.error_message.contains("timed out"),
            "error was: {}",
            result.error_message
        );
        // Should complete within ~3 seconds (2s timeout + tolerance)
        assert!(
            elapsed < Duration::from_secs(5),
            "took too long: {:?}",
            elapsed
        );
    }

    // -- Test 6 (AAPI-06): response_maps_poll_values --

    #[tokio::test]
    async fn response_maps_poll_values() {
        let (base_url, _counter) = start_stateful_server(vec![
            // Submit
            (
                axum::http::StatusCode::OK,
                r#"{"id": "j4"}"#.to_string(),
            ),
            // Poll: completed with nested result
            (
                axum::http::StatusCode::OK,
                r#"{"status": "completed", "result": {"output": "final-output", "code": 0}}"#
                    .to_string(),
            ),
        ])
        .await;

        let async_api = make_async_api(
            &format!("{}/submit", base_url),
            &format!("{}/poll", base_url),
            equal_condition("status", "completed"),
            None,
        );
        let response =
            make_response(r#"output=<poll_response.result.output> code=<poll_response.result.code>"#);
        let executor =
            AsyncApiExecutor::new("test-svc".to_string(), async_api, response, false, false, false).unwrap();
        let assignment = make_assignment("data");

        let result = executor.execute(&assignment).await;
        assert!(result.success, "error: {}", result.error_message);
        assert_eq!(
            result.result.as_str(),
            "output=final-output code=0"
        );
    }

    // -- Test 7: submit_non_2xx_fails --

    #[tokio::test]
    async fn submit_non_2xx_fails() {
        let (base_url, _counter) = start_stateful_server(vec![
            // Submit fails
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal error".to_string(),
            ),
        ])
        .await;

        let async_api = make_async_api(
            &format!("{}/submit", base_url),
            &format!("{}/poll", base_url),
            equal_condition("status", "completed"),
            None,
        );
        let response = make_response("<poll_response.result>");
        let executor =
            AsyncApiExecutor::new("test-svc".to_string(), async_api, response, false, false, false).unwrap();
        let assignment = make_assignment("data");

        let result = executor.execute(&assignment).await;
        assert!(!result.success);
        assert!(
            result.error_message.contains("HTTP 500"),
            "error was: {}",
            result.error_message
        );
    }

    // -- Test 8: poll_non_2xx_fails --

    #[tokio::test]
    async fn poll_non_2xx_fails() {
        let (base_url, _counter) = start_stateful_server(vec![
            // Submit succeeds
            (
                axum::http::StatusCode::OK,
                r#"{"id": "j5"}"#.to_string(),
            ),
            // Poll fails
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "poll error".to_string(),
            ),
        ])
        .await;

        let async_api = make_async_api(
            &format!("{}/submit", base_url),
            &format!("{}/poll", base_url),
            equal_condition("status", "completed"),
            None,
        );
        let response = make_response("<poll_response.result>");
        let executor =
            AsyncApiExecutor::new("test-svc".to_string(), async_api, response, false, false, false).unwrap();
        let assignment = make_assignment("data");

        let result = executor.execute(&assignment).await;
        assert!(!result.success);
        assert!(
            result.error_message.contains("poll HTTP 500"),
            "error was: {}",
            result.error_message
        );
    }

    // -- Test 9: failed_body_template_resolves_on_failure --

    #[tokio::test]
    async fn failed_body_template_resolves_on_failure() {
        let (base_url, _counter) = start_stateful_server(vec![
            // Submit
            (
                axum::http::StatusCode::OK,
                r#"{"id": "j6"}"#.to_string(),
            ),
            // Poll: failed with error details
            (
                axum::http::StatusCode::OK,
                r#"{"status": "failed", "error": {"message": "out of memory", "code": "OOM"}}"#
                    .to_string(),
            ),
        ])
        .await;

        let async_api = make_async_api(
            &format!("{}/submit", base_url),
            &format!("{}/poll", base_url),
            equal_condition("status", "completed"),
            Some(equal_condition("status", "failed")),
        );
        let response = ResponseSection {
            success: SuccessResponseConfig {
                body: "<poll_response.result>".to_string(),
                header: None,
            },
            failed: Some(FailedResponseConfig {
                body: r#"{"err": "<poll_response.error.message>", "code": "<poll_response.error.code>"}"#
                    .to_string(),
                header: None,
            }),
            max_bytes: 1_048_576,
        };
        let executor =
            AsyncApiExecutor::new("test-svc".to_string(), async_api, response, false, false, false).unwrap();
        let assignment = make_assignment("data");

        let result = executor.execute(&assignment).await;
        assert!(!result.success);
        let output = result.result.as_str();
        assert!(
            output.contains("out of memory"),
            "output was: {}",
            output
        );
        assert!(output.contains("OOM"), "output was: {}", output);
    }
}
