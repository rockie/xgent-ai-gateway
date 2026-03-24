//! Tests for EXMP-01, EXMP-02, EXMP-03, EXMP-05:
//! Verifies that example YAML configs in examples/ parse correctly via load_config
//! and that --dry-run exits 0 with a valid-config summary line.

use xgent_gateway::agent::config::{load_config, CliInputMode, ExecutionMode};

/// Helper: path relative to repo root (cargo test cwd = package root = gateway/).
fn example_path(rel: &str) -> String {
    // Cargo sets CARGO_MANIFEST_DIR to the package dir (gateway/).
    // Examples are at <repo_root>/examples/...
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{}/../{}", manifest_dir, rel)
}

// ---------------------------------------------------------------------------
// EXMP-01: CLI example configs parse correctly
// ---------------------------------------------------------------------------

#[test]
fn exmp01_cli_arg_yaml_parses_mode_and_input_mode() {
    let path = example_path("examples/cli-service/agent-arg.yaml");
    let config = load_config(&path).expect("agent-arg.yaml should parse without error");

    assert_eq!(
        config.service.mode,
        ExecutionMode::Cli,
        "agent-arg.yaml must have mode=cli"
    );

    let cli = config
        .cli
        .expect("agent-arg.yaml must have a [cli] section");
    assert_eq!(
        cli.input_mode,
        CliInputMode::Arg,
        "agent-arg.yaml must have input_mode=arg"
    );
    assert!(
        !cli.command.is_empty(),
        "agent-arg.yaml command must be non-empty"
    );
}

#[test]
fn exmp01_cli_stdin_yaml_parses_mode_and_input_mode() {
    let path = example_path("examples/cli-service/agent-stdin.yaml");
    let config = load_config(&path).expect("agent-stdin.yaml should parse without error");

    assert_eq!(
        config.service.mode,
        ExecutionMode::Cli,
        "agent-stdin.yaml must have mode=cli"
    );

    let cli = config
        .cli
        .expect("agent-stdin.yaml must have a [cli] section");
    assert_eq!(
        cli.input_mode,
        CliInputMode::Stdin,
        "agent-stdin.yaml must have input_mode=stdin"
    );
}

// ---------------------------------------------------------------------------
// EXMP-02: Sync-API example config parses correctly
// ---------------------------------------------------------------------------

#[test]
fn exmp02_sync_api_yaml_parses_mode_and_url() {
    let path = example_path("examples/sync-api-service/agent.yaml");
    let config = load_config(&path).expect("sync-api-service/agent.yaml should parse without error");

    assert_eq!(
        config.service.mode,
        ExecutionMode::SyncApi,
        "sync-api agent.yaml must have mode=sync-api"
    );

    let sync_api = config
        .sync_api
        .expect("sync-api agent.yaml must have a [sync_api] section");

    assert!(
        sync_api.url.contains("/sync"),
        "sync-api URL must contain '/sync', got: {}",
        sync_api.url
    );
}

// ---------------------------------------------------------------------------
// EXMP-03: Async-API example config parses correctly
// ---------------------------------------------------------------------------

#[test]
fn exmp03_async_api_yaml_parses_mode_submit_poll_and_completion() {
    let path = example_path("examples/async-api-service/agent.yaml");
    let config =
        load_config(&path).expect("async-api-service/agent.yaml should parse without error");

    assert_eq!(
        config.service.mode,
        ExecutionMode::AsyncApi,
        "async-api agent.yaml must have mode=async-api"
    );

    let async_api = config
        .async_api
        .expect("async-api agent.yaml must have an [async_api] section");

    // Submit URL must reference /async/submit
    assert!(
        async_api.submit.url.contains("/async/submit"),
        "submit URL must contain '/async/submit', got: {}",
        async_api.submit.url
    );

    // Poll URL must contain submit_response placeholder (dynamic job id)
    assert!(
        async_api.poll.url.contains("submit_response"),
        "poll URL must reference submit_response placeholder, got: {}",
        async_api.poll.url
    );

    // completed_when must be present and check status field
    assert_eq!(
        async_api.completed_when.path, "status",
        "completed_when.path must be 'status'"
    );

    // failed_when must be present
    assert!(
        async_api.failed_when.is_some(),
        "async-api agent.yaml must define failed_when"
    );
}

// ---------------------------------------------------------------------------
// EXMP-05: --dry-run exits 0 and prints "Config is valid" for all example configs
// ---------------------------------------------------------------------------

fn agent_binary_path() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    // Cargo places binaries at target/debug/ relative to workspace root.
    // Workspace root is one level above the gateway package (CARGO_MANIFEST_DIR).
    format!("{}/../target/debug/xgent-agent", manifest_dir)
}

fn repo_root() -> String {
    // CARGO_MANIFEST_DIR is the gateway/ package dir; repo root is one level up.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{}/..", manifest_dir)
}

fn run_dry_run(config_rel_path: &str) -> std::process::Output {
    let binary = agent_binary_path();
    let config_path = example_path(config_rel_path);
    // Run from the repo root so that relative command paths in configs (e.g.
    // ./examples/cli-service/echo.sh) resolve correctly -- matching the
    // documented usage pattern where the agent is invoked from the project root.
    std::process::Command::new(&binary)
        .args(["--dry-run", "--config", &config_path])
        .current_dir(repo_root())
        .output()
        .unwrap_or_else(|e| panic!("failed to run xgent-agent binary at {}: {}", binary, e))
}

#[test]
fn exmp05_dry_run_cli_arg_config_exits_0_and_reports_valid() {
    let output = run_dry_run("examples/cli-service/agent-arg.yaml");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "--dry-run on agent-arg.yaml should exit 0\nstdout: {}\nstderr: {}",
        stdout,
        stderr
    );
    assert!(
        stdout.contains("Config is valid"),
        "--dry-run output must contain 'Config is valid'\nstdout: {}",
        stdout
    );
}

#[test]
fn exmp05_dry_run_cli_stdin_config_exits_0_and_reports_valid() {
    let output = run_dry_run("examples/cli-service/agent-stdin.yaml");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "--dry-run on agent-stdin.yaml should exit 0\nstdout: {}\nstderr: {}",
        stdout,
        stderr
    );
    assert!(
        stdout.contains("Config is valid"),
        "--dry-run output must contain 'Config is valid'\nstdout: {}",
        stdout
    );
}

#[test]
fn exmp05_dry_run_sync_api_config_exits_0_and_reports_valid() {
    let output = run_dry_run("examples/sync-api-service/agent.yaml");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "--dry-run on sync-api/agent.yaml should exit 0\nstdout: {}\nstderr: {}",
        stdout,
        stderr
    );
    assert!(
        stdout.contains("Config is valid"),
        "--dry-run output must contain 'Config is valid'\nstdout: {}",
        stdout
    );
}

#[test]
fn exmp05_dry_run_async_api_config_exits_0_and_reports_valid() {
    let output = run_dry_run("examples/async-api-service/agent.yaml");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "--dry-run on async-api/agent.yaml should exit 0\nstdout: {}\nstderr: {}",
        stdout,
        stderr
    );
    assert!(
        stdout.contains("Config is valid"),
        "--dry-run output must contain 'Config is valid'\nstdout: {}",
        stdout
    );
}
