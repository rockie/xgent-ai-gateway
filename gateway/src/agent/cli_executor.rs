use std::collections::HashMap;
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

use super::config::{CliInputMode, CliSection, ResponseSection};
use super::executor::{ExecutionResult, Executor};
use super::placeholder;
use super::response;
use xgent_proto::TaskAssignment;

/// CLI executor that runs child processes in arg or stdin mode.
///
/// - Arg mode: payload is substituted into command template elements via `<payload>`
/// - Stdin mode: payload is piped to process stdin as raw bytes
///
/// Timeout enforcement kills the process via SIGKILL. Exit code 0 = success,
/// non-zero = failure. Response body is built from stdout/stderr via template.
pub struct CliExecutor {
    service_name: String,
    cli: CliSection,
    response: ResponseSection,
}

impl CliExecutor {
    pub fn new(service_name: String, cli: CliSection, response: ResponseSection) -> Self {
        Self {
            service_name,
            cli,
            response,
        }
    }
}

#[async_trait]
impl Executor for CliExecutor {
    async fn execute(&self, assignment: &TaskAssignment) -> ExecutionResult {
        // (a) Build task variables
        let mut variables = placeholder::build_task_variables(assignment, &self.service_name);

        // (b) Resolve placeholders in each command element
        let mut resolved_command = Vec::with_capacity(self.cli.command.len());
        for element in &self.cli.command {
            match placeholder::resolve_placeholders(element, &variables) {
                Ok(resolved) => resolved_command.push(resolved),
                Err(e) => {
                    return ExecutionResult {
                        success: false,
                        result: Vec::new(),
                        error_message: format!("failed to resolve command placeholder: {}", e),
                    };
                }
            }
        }

        if resolved_command.is_empty() {
            return ExecutionResult {
                success: false,
                result: Vec::new(),
                error_message: "command list is empty".to_string(),
            };
        }

        // (c) Build the Command
        let program = &resolved_command[0];
        let args = &resolved_command[1..];

        let mut cmd = Command::new(program);
        cmd.args(args);
        cmd.kill_on_drop(true);

        // (d) Set working directory if configured
        if let Some(ref cwd) = self.cli.cwd {
            cmd.current_dir(cwd);
        }

        // (e) Inject environment variables
        for (k, v) in &self.cli.env {
            cmd.env(k, v);
        }

        // Configure stdio based on input mode
        match self.cli.input_mode {
            CliInputMode::Stdin => {
                cmd.stdin(Stdio::piped());
            }
            CliInputMode::Arg => {
                cmd.stdin(Stdio::null());
            }
        }
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // Spawn the process
        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) => {
                return ExecutionResult {
                    success: false,
                    result: Vec::new(),
                    error_message: format!("failed to spawn process: {}", e),
                };
            }
        };

        // (f/g) Set up concurrent I/O
        let stdin_task = if self.cli.input_mode == CliInputMode::Stdin {
            let mut stdin_handle = child.stdin.take().expect("stdin was piped");
            let payload = assignment.payload.clone();
            Some(tokio::spawn(async move {
                let _ = stdin_handle.write_all(&payload).await;
                let _ = stdin_handle.shutdown().await;
            }))
        } else {
            None
        };

        let mut stdout_handle = child.stdout.take().expect("stdout was piped");
        let mut stderr_handle = child.stderr.take().expect("stderr was piped");

        let stdout_task = tokio::spawn(async move {
            let mut buf = Vec::new();
            stdout_handle.read_to_end(&mut buf).await.map(|_| buf)
        });

        let stderr_task = tokio::spawn(async move {
            let mut buf = Vec::new();
            stderr_handle.read_to_end(&mut buf).await.map(|_| buf)
        });

        // (h) Wait for process with timeout
        let timeout_duration = Duration::from_secs(self.cli.timeout_secs);
        let wait_result = tokio::time::timeout(timeout_duration, child.wait()).await;

        match wait_result {
            Err(_) => {
                // Timeout expired -- explicitly kill the child process (D-12)
                let _ = child.kill().await;
                // Wait for I/O tasks to finish
                if let Some(task) = stdin_task {
                    let _ = task.await;
                }
                let _ = stdout_task.await;
                let _ = stderr_task.await;
                return ExecutionResult {
                    success: false,
                    result: Vec::new(),
                    error_message: format!(
                        "process timed out after {} seconds",
                        self.cli.timeout_secs
                    ),
                };
            }
            Ok(Err(e)) => {
                if let Some(task) = stdin_task {
                    let _ = task.await;
                }
                let _ = stdout_task.await;
                let _ = stderr_task.await;
                return ExecutionResult {
                    success: false,
                    result: Vec::new(),
                    error_message: format!("failed to wait for process: {}", e),
                };
            }
            Ok(Ok(status)) => {
                // (i) Collect stdout and stderr from spawned tasks
                if let Some(task) = stdin_task {
                    let _ = task.await;
                }

                let stdout_bytes = match stdout_task.await {
                    Ok(Ok(bytes)) => bytes,
                    Ok(Err(e)) => {
                        return ExecutionResult {
                            success: false,
                            result: Vec::new(),
                            error_message: format!("stdout read error: {}", e),
                        };
                    }
                    Err(e) => {
                        return ExecutionResult {
                            success: false,
                            result: Vec::new(),
                            error_message: format!("stdout task panicked: {}", e),
                        };
                    }
                };

                let stderr_bytes = match stderr_task.await {
                    Ok(Ok(bytes)) => bytes,
                    Ok(Err(e)) => {
                        return ExecutionResult {
                            success: false,
                            result: Vec::new(),
                            error_message: format!("stderr read error: {}", e),
                        };
                    }
                    Err(e) => {
                        return ExecutionResult {
                            success: false,
                            result: Vec::new(),
                            error_message: format!("stderr task panicked: {}", e),
                        };
                    }
                };

                // (j) Check exit code
                let exit_code = status.code().unwrap_or(-1);
                if exit_code != 0 {
                    return ExecutionResult {
                        success: false,
                        result: Vec::new(),
                        error_message: format!("process exited with code {}", exit_code),
                    };
                }

                // (k) Add stdout and stderr to variables
                let stdout_str = String::from_utf8_lossy(&stdout_bytes).to_string();
                let stderr_str = String::from_utf8_lossy(&stderr_bytes).to_string();
                variables.insert("stdout".to_string(), stdout_str);
                variables.insert("stderr".to_string(), stderr_str);

                // (l) Resolve response body template
                match response::resolve_response_body(
                    &self.response.body,
                    &variables,
                    self.response.max_bytes,
                ) {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_assignment(payload: &[u8]) -> TaskAssignment {
        TaskAssignment {
            task_id: "test-task-1".to_string(),
            payload: payload.to_vec(),
            metadata: HashMap::new(),
        }
    }

    fn make_assignment_with_metadata(
        payload: &[u8],
        metadata: HashMap<String, String>,
    ) -> TaskAssignment {
        TaskAssignment {
            task_id: "test-task-1".to_string(),
            payload: payload.to_vec(),
            metadata,
        }
    }

    fn make_cli(
        command: Vec<&str>,
        input_mode: CliInputMode,
        timeout_secs: u64,
    ) -> CliSection {
        CliSection {
            command: command.into_iter().map(String::from).collect(),
            input_mode,
            timeout_secs,
            cwd: None,
            env: HashMap::new(),
        }
    }

    fn make_response(body: &str) -> ResponseSection {
        ResponseSection {
            body: body.to_string(),
            max_bytes: 1_048_576,
        }
    }

    // -- Arg mode tests --

    #[tokio::test]
    async fn arg_mode_echo_payload() {
        let cli = make_cli(vec!["echo", "<payload>"], CliInputMode::Arg, 10);
        let response = make_response("<stdout>");
        let executor = CliExecutor::new("test-svc".to_string(), cli, response);
        let assignment = make_assignment(b"hello");

        let result = executor.execute(&assignment).await;
        assert!(result.success, "error: {}", result.error_message);
        assert_eq!(String::from_utf8_lossy(&result.result).trim(), "hello");
    }

    #[tokio::test]
    async fn arg_mode_payload_in_flag() {
        let cli = make_cli(vec!["echo", "--data=<payload>"], CliInputMode::Arg, 10);
        let response = make_response("<stdout>");
        let executor = CliExecutor::new("test-svc".to_string(), cli, response);
        let assignment = make_assignment(b"test");

        let result = executor.execute(&assignment).await;
        assert!(result.success, "error: {}", result.error_message);
        assert_eq!(
            String::from_utf8_lossy(&result.result).trim(),
            "--data=test"
        );
    }

    #[tokio::test]
    async fn arg_mode_service_name_placeholder() {
        let cli = make_cli(vec!["echo", "<service_name>"], CliInputMode::Arg, 10);
        let response = make_response("<stdout>");
        let executor = CliExecutor::new("my-service".to_string(), cli, response);
        let assignment = make_assignment(b"ignored");

        let result = executor.execute(&assignment).await;
        assert!(result.success, "error: {}", result.error_message);
        assert_eq!(
            String::from_utf8_lossy(&result.result).trim(),
            "my-service"
        );
    }

    #[tokio::test]
    async fn arg_mode_metadata_placeholder() {
        let cli = make_cli(vec!["echo", "<metadata.region>"], CliInputMode::Arg, 10);
        let response = make_response("<stdout>");
        let executor = CliExecutor::new("test-svc".to_string(), cli, response);

        let mut metadata = HashMap::new();
        metadata.insert("region".to_string(), "us-east-1".to_string());
        let assignment = make_assignment_with_metadata(b"data", metadata);

        let result = executor.execute(&assignment).await;
        assert!(result.success, "error: {}", result.error_message);
        assert_eq!(
            String::from_utf8_lossy(&result.result).trim(),
            "us-east-1"
        );
    }

    // -- Stdin mode tests --

    #[tokio::test]
    async fn stdin_mode_cat_payload() {
        let cli = make_cli(vec!["cat"], CliInputMode::Stdin, 10);
        let response = make_response("<stdout>");
        let executor = CliExecutor::new("test-svc".to_string(), cli, response);
        let assignment = make_assignment(b"hello");

        let result = executor.execute(&assignment).await;
        assert!(result.success, "error: {}", result.error_message);
        assert_eq!(String::from_utf8_lossy(&result.result), "hello");
    }

    #[tokio::test]
    async fn stdin_mode_large_payload_no_deadlock() {
        // >100KB payload to exercise concurrent I/O (pipe buffers are typically 64KB)
        let payload = vec![b'X'; 128 * 1024];
        let cli = make_cli(vec!["cat"], CliInputMode::Stdin, 30);
        let response = ResponseSection {
            body: "<stdout>".to_string(),
            max_bytes: 256 * 1024,
        };
        let executor = CliExecutor::new("test-svc".to_string(), cli, response);
        let assignment = make_assignment(&payload);

        let result = executor.execute(&assignment).await;
        assert!(result.success, "error: {}", result.error_message);
        assert_eq!(result.result.len(), 128 * 1024);
    }

    // -- Timeout tests --

    #[tokio::test]
    async fn timeout_kills_process() {
        let cli = make_cli(vec!["sleep", "60"], CliInputMode::Arg, 1);
        let response = make_response("<stdout>");
        let executor = CliExecutor::new("test-svc".to_string(), cli, response);
        let assignment = make_assignment(b"");

        let result = executor.execute(&assignment).await;
        assert!(!result.success);
        assert!(
            result.error_message.contains("timed out"),
            "error was: {}",
            result.error_message
        );
    }

    #[tokio::test]
    async fn timeout_process_is_actually_killed() {
        // Spawn a process that would run for 60 seconds, timeout after 1 second,
        // then verify it was killed by checking we can complete quickly
        let cli = make_cli(vec!["sleep", "60"], CliInputMode::Arg, 1);
        let response = make_response("<stdout>");
        let executor = CliExecutor::new("test-svc".to_string(), cli, response);
        let assignment = make_assignment(b"");

        let start = std::time::Instant::now();
        let result = executor.execute(&assignment).await;
        let elapsed = start.elapsed();

        assert!(!result.success);
        // Should complete in ~1-2 seconds, not 60
        assert!(
            elapsed.as_secs() < 5,
            "took too long ({:?}), process may not have been killed",
            elapsed
        );
    }

    // -- Exit code tests --

    #[tokio::test]
    async fn exit_code_zero_is_success() {
        let cli = make_cli(vec!["true"], CliInputMode::Arg, 10);
        let response = make_response("<stdout>");
        let executor = CliExecutor::new("test-svc".to_string(), cli, response);
        let assignment = make_assignment(b"");

        let result = executor.execute(&assignment).await;
        assert!(result.success, "error: {}", result.error_message);
    }

    #[tokio::test]
    async fn exit_code_nonzero_is_failure() {
        let cli = make_cli(vec!["false"], CliInputMode::Arg, 10);
        let response = make_response("<stdout>");
        let executor = CliExecutor::new("test-svc".to_string(), cli, response);
        let assignment = make_assignment(b"");

        let result = executor.execute(&assignment).await;
        assert!(!result.success);
        assert!(
            result.error_message.contains("exited with code"),
            "error was: {}",
            result.error_message
        );
    }

    // -- Process environment tests --

    #[tokio::test]
    async fn cwd_sets_working_directory() {
        let mut cli = make_cli(vec!["pwd"], CliInputMode::Arg, 10);
        cli.cwd = Some("/tmp".to_string());
        let response = make_response("<stdout>");
        let executor = CliExecutor::new("test-svc".to_string(), cli, response);
        let assignment = make_assignment(b"");

        let result = executor.execute(&assignment).await;
        assert!(result.success, "error: {}", result.error_message);
        let output = String::from_utf8_lossy(&result.result);
        // On macOS /tmp -> /private/tmp
        assert!(
            output.contains("/tmp"),
            "output was: {}",
            output
        );
    }

    #[tokio::test]
    async fn env_vars_injected_into_process() {
        let mut cli = make_cli(vec!["printenv", "TEST_CLI_VAR"], CliInputMode::Arg, 10);
        cli.env
            .insert("TEST_CLI_VAR".to_string(), "hello-env".to_string());
        let response = make_response("<stdout>");
        let executor = CliExecutor::new("test-svc".to_string(), cli, response);
        let assignment = make_assignment(b"");

        let result = executor.execute(&assignment).await;
        assert!(result.success, "error: {}", result.error_message);
        assert_eq!(
            String::from_utf8_lossy(&result.result).trim(),
            "hello-env"
        );
    }

    // -- Response template integration --

    #[tokio::test]
    async fn response_template_resolves_stdout_stderr() {
        let cli = make_cli(vec!["echo", "output-data"], CliInputMode::Arg, 10);
        let response = make_response(r#"{"out": "<stdout>", "err": "<stderr>"}"#);
        let executor = CliExecutor::new("test-svc".to_string(), cli, response);
        let assignment = make_assignment(b"");

        let result = executor.execute(&assignment).await;
        assert!(result.success, "error: {}", result.error_message);
        let output = String::from_utf8_lossy(&result.result);
        assert!(
            output.contains("output-data"),
            "output was: {}",
            output
        );
        assert!(
            output.contains(r#""err": ""#),
            "output was: {}",
            output
        );
    }
}
