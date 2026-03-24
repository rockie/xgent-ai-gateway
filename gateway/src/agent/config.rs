use std::collections::HashMap;

use serde::Deserialize;

/// Top-level agent configuration loaded from agent.yaml.
#[derive(Debug, Deserialize)]
pub struct AgentConfig {
    pub gateway: GatewaySection,
    pub service: ServiceSection,
    #[serde(default)]
    pub cli: Option<CliSection>,
    #[serde(default)]
    pub sync_api: Option<SyncApiSection>,
    #[serde(default)]
    pub async_api: Option<AsyncApiSection>,
    pub response: ResponseSection,
}

/// Gateway connection configuration.
#[derive(Debug, Deserialize)]
pub struct GatewaySection {
    pub address: String,
    pub token: String,
    #[serde(default = "default_node_id")]
    pub node_id: String,
    pub ca_cert: Option<String>,
    #[serde(default)]
    pub tls_skip_verify: bool,
    #[serde(default = "default_max_reconnect_delay_secs")]
    pub max_reconnect_delay_secs: u64,
}

/// Service identity and execution mode.
#[derive(Debug, Deserialize)]
pub struct ServiceSection {
    pub name: String,
    pub mode: ExecutionMode,
}

/// Execution mode for the agent.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionMode {
    Cli,
    SyncApi,
    AsyncApi,
}

/// CLI execution configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct CliSection {
    pub command: Vec<String>,
    #[serde(default = "default_input_mode")]
    pub input_mode: CliInputMode,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// How the task payload is passed to the CLI process.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CliInputMode {
    Arg,
    Stdin,
}

/// Sync-API HTTP execution configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct SyncApiSection {
    pub url: String,
    #[serde(default = "default_http_method")]
    pub method: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    #[serde(default = "default_sync_api_timeout")]
    pub timeout_secs: u64,
    #[serde(default)]
    pub tls_skip_verify: bool,
}

/// Async-API two-phase HTTP execution configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct AsyncApiSection {
    pub submit: SubmitSection,
    pub poll: PollSection,
    pub completed_when: CompletionCondition,
    #[serde(default)]
    pub failed_when: Option<CompletionCondition>,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    #[serde(default)]
    pub tls_skip_verify: bool,
}

/// Submit phase configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct SubmitSection {
    pub url: String,
    #[serde(default = "default_http_method")]
    pub method: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

/// Poll phase configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct PollSection {
    pub url: String,
    #[serde(default = "default_poll_method")]
    pub method: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    #[serde(default = "default_poll_interval")]
    pub interval_secs: u64,
}

/// Condition for completion or failure detection.
#[derive(Debug, Clone, Deserialize)]
pub struct CompletionCondition {
    pub path: String,
    pub operator: ConditionOperator,
    pub value: ConditionValue,
}

/// Operator for condition evaluation.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionOperator {
    Equal,
    NotEqual,
    In,
    NotIn,
}

/// Condition value: single string for equal/not_equal, array for in/not_in.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ConditionValue {
    Single(String),
    Multiple(Vec<String>),
}

impl CompletionCondition {
    /// Evaluate the condition against a JSON response.
    /// Extracts the value at `self.path` and compares using `self.operator`.
    pub fn evaluate(&self, json: &serde_json::Value) -> Result<bool, String> {
        let actual = super::http_common::extract_json_value(json, &self.path)?;
        match (&self.operator, &self.value) {
            (ConditionOperator::Equal, ConditionValue::Single(expected)) => {
                Ok(actual == *expected)
            }
            (ConditionOperator::NotEqual, ConditionValue::Single(expected)) => {
                Ok(actual != *expected)
            }
            (ConditionOperator::In, ConditionValue::Multiple(values)) => {
                Ok(values.iter().any(|v| v == &actual))
            }
            (ConditionOperator::NotIn, ConditionValue::Multiple(values)) => {
                Ok(!values.iter().any(|v| v == &actual))
            }
            (ConditionOperator::Equal | ConditionOperator::NotEqual, ConditionValue::Multiple(_)) => {
                Err(format!(
                    "operator {:?} requires a single string value, not an array",
                    self.operator
                ))
            }
            (ConditionOperator::In | ConditionOperator::NotIn, ConditionValue::Single(_)) => {
                Err(format!(
                    "operator {:?} requires an array value, not a single string",
                    self.operator
                ))
            }
        }
    }
}

/// Response body template configuration with success/failed sub-sections.
#[derive(Debug, Clone, Deserialize)]
pub struct ResponseSection {
    pub success: SuccessResponseConfig,
    #[serde(default)]
    pub failed: Option<FailedResponseConfig>,
    #[serde(default = "default_max_bytes")]
    pub max_bytes: usize,
}

/// Configuration for the success response body template and optional headers.
#[derive(Debug, Clone, Deserialize)]
pub struct SuccessResponseConfig {
    pub body: String,
    #[serde(default)]
    pub header: Option<String>,
}

/// Configuration for the failed response body template and optional headers.
#[derive(Debug, Clone, Deserialize)]
pub struct FailedResponseConfig {
    pub body: String,
    #[serde(default)]
    pub header: Option<String>,
}

fn default_node_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

fn default_max_reconnect_delay_secs() -> u64 {
    30
}

fn default_input_mode() -> CliInputMode {
    CliInputMode::Arg
}

fn default_timeout_secs() -> u64 {
    300
}

fn default_http_method() -> String {
    "POST".to_string()
}

fn default_poll_method() -> String {
    "GET".to_string()
}

fn default_poll_interval() -> u64 {
    5
}

fn default_sync_api_timeout() -> u64 {
    30
}

fn default_max_bytes() -> usize {
    1_048_576
}

/// Load and parse agent configuration from a YAML file.
///
/// Steps:
/// 1. Read file to string
/// 2. Interpolate `${ENV_VAR}` patterns from environment
/// 3. Parse YAML into AgentConfig
/// 4. Validate: if mode is Cli, cli section must be present
pub fn load_config(path: &str) -> Result<AgentConfig, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read config file '{}': {}", path, e))?;
    load_config_from_str(&raw)
}

/// Load and parse agent configuration from a YAML string.
/// Useful for testing without needing a file on disk.
pub fn load_config_from_str(raw: &str) -> Result<AgentConfig, String> {
    let interpolated = interpolate_env_vars(raw)?;
    let config: AgentConfig = serde_yaml_ng::from_str(&interpolated)
        .map_err(|e| format!("failed to parse agent config YAML: {}", e))?;

    // Validate: CLI mode requires cli section
    if config.service.mode == ExecutionMode::Cli && config.cli.is_none() {
        return Err("mode is 'cli' but [cli] section is missing".to_string());
    }

    // Validate: sync-api mode requires sync_api section
    if config.service.mode == ExecutionMode::SyncApi && config.sync_api.is_none() {
        return Err("mode is 'sync-api' but [sync_api] section is missing".to_string());
    }

    // Validate: async-api mode requires async_api section
    if config.service.mode == ExecutionMode::AsyncApi && config.async_api.is_none() {
        return Err("mode is 'async-api' but [async_api] section is missing".to_string());
    }

    Ok(config)
}

/// Resolve `${ENV_VAR}` patterns in a raw string from environment variables.
/// Missing environment variables cause an immediate error.
fn interpolate_env_vars(raw: &str) -> Result<String, String> {
    let mut result = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '$' && chars.peek() == Some(&'{') {
            // Consume the '{'
            chars.next();
            let mut var_name = String::new();
            let mut found_close = false;
            for c2 in chars.by_ref() {
                if c2 == '}' {
                    found_close = true;
                    break;
                }
                var_name.push(c2);
            }
            if found_close {
                let value = std::env::var(&var_name).map_err(|_| {
                    format!("missing environment variable: ${{{}}}", var_name)
                })?;
                result.push_str(&value);
            } else {
                // No closing brace -- preserve literal
                result.push('$');
                result.push('{');
                result.push_str(&var_name);
            }
        } else {
            result.push(c);
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serialize tests that touch env vars to prevent races.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn valid_yaml() -> &'static str {
        r#"
gateway:
  address: "localhost:50051"
  token: "my-token"
  node_id: "node-01"
  tls_skip_verify: false
  max_reconnect_delay_secs: 30

service:
  name: "test-service"
  mode: cli

cli:
  command: ["echo", "hello"]
  input_mode: arg
  timeout_secs: 60

response:
  success:
    body: '{"output": "<stdout>"}'
  max_bytes: 2048
"#
    }

    #[test]
    fn valid_yaml_parses_all_sections() {
        let config = load_config_from_str(valid_yaml()).unwrap();
        assert_eq!(config.gateway.address, "localhost:50051");
        assert_eq!(config.gateway.token, "my-token");
        assert_eq!(config.gateway.node_id, "node-01");
        assert!(!config.gateway.tls_skip_verify);
        assert_eq!(config.gateway.max_reconnect_delay_secs, 30);

        assert_eq!(config.service.name, "test-service");
        assert_eq!(config.service.mode, ExecutionMode::Cli);

        let cli = config.cli.unwrap();
        assert_eq!(cli.command, vec!["echo", "hello"]);
        assert_eq!(cli.input_mode, CliInputMode::Arg);
        assert_eq!(cli.timeout_secs, 60);

        assert_eq!(config.response.success.body, "{\"output\": \"<stdout>\"}");
        assert_eq!(config.response.max_bytes, 2048);
    }

    #[test]
    fn env_var_interpolation_resolves() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("AGENT_TEST_TOKEN", "secret-token-123");

        let yaml = r#"
gateway:
  address: "localhost:50051"
  token: "${AGENT_TEST_TOKEN}"
  node_id: "node-01"

service:
  name: "test-service"
  mode: cli

cli:
  command: ["echo"]

response:
  success:
    body: "<stdout>"
"#;
        let config = load_config_from_str(yaml).unwrap();
        assert_eq!(config.gateway.token, "secret-token-123");

        std::env::remove_var("AGENT_TEST_TOKEN");
    }

    #[test]
    fn missing_env_var_causes_error() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("AGENT_MISSING_VAR_TEST");

        let yaml = r#"
gateway:
  address: "localhost:50051"
  token: "${AGENT_MISSING_VAR_TEST}"
  node_id: "node-01"

service:
  name: "test-service"
  mode: cli

cli:
  command: ["echo"]

response:
  success:
    body: "<stdout>"
"#;
        let err = load_config_from_str(yaml).unwrap_err();
        assert!(
            err.contains("missing environment variable: ${AGENT_MISSING_VAR_TEST}"),
            "error was: {}",
            err
        );
    }

    #[test]
    fn mode_cli_deserializes() {
        let config = load_config_from_str(valid_yaml()).unwrap();
        assert_eq!(config.service.mode, ExecutionMode::Cli);
    }

    #[test]
    fn mode_sync_api_deserializes() {
        let yaml = r#"
gateway:
  address: "localhost:50051"
  token: "tok"

service:
  name: "test"
  mode: sync-api

sync_api:
  url: "http://localhost:8080/api"

response:
  success:
    body: "<stdout>"
"#;
        let config = load_config_from_str(yaml).unwrap();
        assert_eq!(config.service.mode, ExecutionMode::SyncApi);
    }

    #[test]
    fn mode_async_api_deserializes() {
        let yaml = r#"
gateway:
  address: "localhost:50051"
  token: "tok"

service:
  name: "test"
  mode: async-api

async_api:
  submit:
    url: "http://example.com/jobs"
  poll:
    url: "http://example.com/jobs/status"
  completed_when:
    path: "status"
    operator: equal
    value: "done"

response:
  success:
    body: "<poll_response.result>"
"#;
        let config = load_config_from_str(yaml).unwrap();
        assert_eq!(config.service.mode, ExecutionMode::AsyncApi);
    }

    #[test]
    fn default_values_apply() {
        let yaml = r#"
gateway:
  address: "localhost:50051"
  token: "tok"

service:
  name: "test"
  mode: cli

cli:
  command: ["echo"]

response:
  success:
    body: "<stdout>"
"#;
        let config = load_config_from_str(yaml).unwrap();
        // Default timeout
        assert_eq!(config.cli.as_ref().unwrap().timeout_secs, 300);
        // Default max_bytes
        assert_eq!(config.response.max_bytes, 1_048_576);
        // Default input_mode
        assert_eq!(config.cli.as_ref().unwrap().input_mode, CliInputMode::Arg);
        // Default max_reconnect_delay_secs
        assert_eq!(config.gateway.max_reconnect_delay_secs, 30);
        // Default tls_skip_verify
        assert!(!config.gateway.tls_skip_verify);
    }

    #[test]
    fn cli_section_with_cwd_and_env() {
        let yaml = r#"
gateway:
  address: "localhost:50051"
  token: "tok"

service:
  name: "test"
  mode: cli

cli:
  command: ["python", "script.py"]
  cwd: "/opt/scripts"
  env:
    PYTHONPATH: "/opt/lib"
    API_KEY: "test-key"

response:
  success:
    body: "<stdout>"
"#;
        let config = load_config_from_str(yaml).unwrap();
        let cli = config.cli.unwrap();
        assert_eq!(cli.cwd, Some("/opt/scripts".to_string()));
        assert_eq!(cli.env.get("PYTHONPATH").unwrap(), "/opt/lib");
        assert_eq!(cli.env.get("API_KEY").unwrap(), "test-key");
        assert_eq!(cli.env.len(), 2);
    }

    #[test]
    fn cli_mode_without_cli_section_fails() {
        let yaml = r#"
gateway:
  address: "localhost:50051"
  token: "tok"

service:
  name: "test"
  mode: cli

response:
  success:
    body: "<stdout>"
"#;
        let err = load_config_from_str(yaml).unwrap_err();
        assert!(err.contains("cli"), "error was: {}", err);
    }

    #[test]
    fn sync_api_yaml_parses_all_fields() {
        let yaml = r#"
gateway:
  address: "localhost:50051"
  token: "tok"

service:
  name: "test"
  mode: sync-api

sync_api:
  url: "http://example.com/api/v1/run"
  method: "PUT"
  headers:
    Authorization: "Bearer abc123"
    Content-Type: "application/json"
  body: '{"input": "<payload>"}'
  timeout_secs: 45
  tls_skip_verify: true

response:
  success:
    body: "<response.result>"
"#;
        let config = load_config_from_str(yaml).unwrap();
        let sa = config.sync_api.unwrap();
        assert_eq!(sa.url, "http://example.com/api/v1/run");
        assert_eq!(sa.method, "PUT");
        assert_eq!(sa.headers.get("Authorization").unwrap(), "Bearer abc123");
        assert_eq!(sa.headers.get("Content-Type").unwrap(), "application/json");
        assert_eq!(sa.body.as_deref(), Some(r#"{"input": "<payload>"}"#));
        assert_eq!(sa.timeout_secs, 45);
        assert!(sa.tls_skip_verify);
    }

    #[test]
    fn sync_api_defaults_apply() {
        let yaml = r#"
gateway:
  address: "localhost:50051"
  token: "tok"

service:
  name: "test"
  mode: sync-api

sync_api:
  url: "http://localhost:8080/api"

response:
  success:
    body: "<response.output>"
"#;
        let config = load_config_from_str(yaml).unwrap();
        let sa = config.sync_api.unwrap();
        assert_eq!(sa.method, "POST");
        assert_eq!(sa.timeout_secs, 30);
        assert!(!sa.tls_skip_verify);
        assert!(sa.headers.is_empty());
        assert!(sa.body.is_none());
    }

    #[test]
    fn sync_api_mode_without_section_fails() {
        let yaml = r#"
gateway:
  address: "localhost:50051"
  token: "tok"

service:
  name: "test"
  mode: sync-api

response:
  success:
    body: "<stdout>"
"#;
        let err = load_config_from_str(yaml).unwrap_err();
        assert!(err.contains("sync_api"), "error was: {}", err);
        assert!(err.contains("missing"), "error was: {}", err);
    }

    #[test]
    fn sync_api_mode_with_section_passes() {
        let yaml = r#"
gateway:
  address: "localhost:50051"
  token: "tok"

service:
  name: "test"
  mode: sync-api

sync_api:
  url: "http://localhost:8080/run"

response:
  success:
    body: "<response.output>"
"#;
        let config = load_config_from_str(yaml).unwrap();
        assert_eq!(config.service.mode, ExecutionMode::SyncApi);
        assert!(config.sync_api.is_some());
    }

    #[test]
    fn async_api_yaml_parses_all_fields() {
        let yaml = r#"
gateway:
  address: "localhost:50051"
  token: "tok"

service:
  name: "test"
  mode: async-api

async_api:
  timeout_secs: 120
  tls_skip_verify: true
  submit:
    url: "http://example.com/jobs"
    method: "POST"
    headers:
      Content-Type: "application/json"
    body: '{"input": "<payload>"}'
  poll:
    url: "http://example.com/jobs/<submit_response.id>/status"
    method: "GET"
    interval_secs: 10
  completed_when:
    path: "status"
    operator: equal
    value: "completed"
  failed_when:
    path: "status"
    operator: in
    value: ["failed", "error", "cancelled"]

response:
  success:
    body: '{"result": "<poll_response.result>"}'
  max_bytes: 1048576
"#;
        let config = load_config_from_str(yaml).unwrap();
        assert_eq!(config.service.mode, ExecutionMode::AsyncApi);

        let aa = config.async_api.unwrap();
        assert_eq!(aa.timeout_secs, 120);
        assert!(aa.tls_skip_verify);

        // Submit
        assert_eq!(aa.submit.url, "http://example.com/jobs");
        assert_eq!(aa.submit.method, "POST");
        assert_eq!(
            aa.submit.headers.get("Content-Type").unwrap(),
            "application/json"
        );
        assert_eq!(
            aa.submit.body.as_deref(),
            Some(r#"{"input": "<payload>"}"#)
        );

        // Poll
        assert_eq!(
            aa.poll.url,
            "http://example.com/jobs/<submit_response.id>/status"
        );
        assert_eq!(aa.poll.method, "GET");
        assert_eq!(aa.poll.interval_secs, 10);

        // completed_when
        assert_eq!(aa.completed_when.path, "status");
        matches!(aa.completed_when.operator, ConditionOperator::Equal);
        matches!(
            aa.completed_when.value,
            ConditionValue::Single(ref s) if s == "completed"
        );

        // failed_when
        let fw = aa.failed_when.unwrap();
        assert_eq!(fw.path, "status");
        matches!(fw.operator, ConditionOperator::In);
        matches!(fw.value, ConditionValue::Multiple(ref v) if v.len() == 3);
    }

    #[test]
    fn async_api_defaults_apply() {
        let yaml = r#"
gateway:
  address: "localhost:50051"
  token: "tok"

service:
  name: "test"
  mode: async-api

async_api:
  submit:
    url: "http://example.com/jobs"
  poll:
    url: "http://example.com/jobs/status"
  completed_when:
    path: "status"
    operator: equal
    value: "done"

response:
  success:
    body: "<poll_response.result>"
"#;
        let config = load_config_from_str(yaml).unwrap();
        let aa = config.async_api.unwrap();
        assert_eq!(aa.submit.method, "POST");
        assert_eq!(aa.poll.method, "GET");
        assert_eq!(aa.poll.interval_secs, 5);
        assert_eq!(aa.timeout_secs, 300);
        assert!(!aa.tls_skip_verify);
    }

    #[test]
    fn async_api_mode_without_section_fails() {
        let yaml = r#"
gateway:
  address: "localhost:50051"
  token: "tok"

service:
  name: "test"
  mode: async-api

response:
  success:
    body: "<stdout>"
"#;
        let err = load_config_from_str(yaml).unwrap_err();
        assert!(err.contains("async_api"), "error was: {}", err);
        assert!(err.contains("missing"), "error was: {}", err);
    }

    #[test]
    fn condition_equal_evaluates() {
        let json: serde_json::Value =
            serde_json::from_str(r#"{"status": "completed"}"#).unwrap();
        let cond = CompletionCondition {
            path: "status".to_string(),
            operator: ConditionOperator::Equal,
            value: ConditionValue::Single("completed".to_string()),
        };
        assert!(cond.evaluate(&json).unwrap());

        let cond_no_match = CompletionCondition {
            path: "status".to_string(),
            operator: ConditionOperator::Equal,
            value: ConditionValue::Single("running".to_string()),
        };
        assert!(!cond_no_match.evaluate(&json).unwrap());
    }

    #[test]
    fn condition_not_equal_evaluates() {
        let json: serde_json::Value =
            serde_json::from_str(r#"{"status": "running"}"#).unwrap();
        let cond = CompletionCondition {
            path: "status".to_string(),
            operator: ConditionOperator::NotEqual,
            value: ConditionValue::Single("completed".to_string()),
        };
        assert!(cond.evaluate(&json).unwrap());

        let cond_match = CompletionCondition {
            path: "status".to_string(),
            operator: ConditionOperator::NotEqual,
            value: ConditionValue::Single("running".to_string()),
        };
        assert!(!cond_match.evaluate(&json).unwrap());
    }

    #[test]
    fn condition_in_evaluates() {
        let json: serde_json::Value =
            serde_json::from_str(r#"{"status": "failed"}"#).unwrap();
        let cond = CompletionCondition {
            path: "status".to_string(),
            operator: ConditionOperator::In,
            value: ConditionValue::Multiple(vec![
                "failed".to_string(),
                "error".to_string(),
                "cancelled".to_string(),
            ]),
        };
        assert!(cond.evaluate(&json).unwrap());

        let json_no_match: serde_json::Value =
            serde_json::from_str(r#"{"status": "running"}"#).unwrap();
        assert!(!cond.evaluate(&json_no_match).unwrap());
    }

    #[test]
    fn condition_not_in_evaluates() {
        let json: serde_json::Value =
            serde_json::from_str(r#"{"status": "running"}"#).unwrap();
        let cond = CompletionCondition {
            path: "status".to_string(),
            operator: ConditionOperator::NotIn,
            value: ConditionValue::Multiple(vec![
                "failed".to_string(),
                "error".to_string(),
            ]),
        };
        assert!(cond.evaluate(&json).unwrap());

        let json_match: serde_json::Value =
            serde_json::from_str(r#"{"status": "failed"}"#).unwrap();
        assert!(!cond.evaluate(&json_match).unwrap());
    }

    #[test]
    fn condition_value_deserializes_single() {
        let yaml = r#"
path: "status"
operator: equal
value: "done"
"#;
        let cond: CompletionCondition = serde_yaml_ng::from_str(yaml).unwrap();
        matches!(cond.value, ConditionValue::Single(ref s) if s == "done");
    }

    #[test]
    fn condition_value_deserializes_multiple() {
        let yaml = r#"
path: "status"
operator: in
value: ["a", "b", "c"]
"#;
        let cond: CompletionCondition = serde_yaml_ng::from_str(yaml).unwrap();
        if let ConditionValue::Multiple(ref v) = cond.value {
            assert_eq!(v.len(), 3);
            assert_eq!(v[0], "a");
            assert_eq!(v[1], "b");
            assert_eq!(v[2], "c");
        } else {
            panic!("expected Multiple variant");
        }
    }
}
