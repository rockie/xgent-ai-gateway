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

/// Response body template configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ResponseSection {
    pub body: String,
    #[serde(default = "default_max_bytes")]
    pub max_bytes: usize,
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

        assert_eq!(config.response.body, "{\"output\": \"<stdout>\"}");
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

response:
  body: "<stdout>"
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
  body: "<response.output>"
"#;
        let config = load_config_from_str(yaml).unwrap();
        assert_eq!(config.service.mode, ExecutionMode::SyncApi);
        assert!(config.sync_api.is_some());
    }
}
