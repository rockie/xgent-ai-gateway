//! xgent-agent: Lightweight runner agent (node-side proxy) that connects to the
//! gateway via gRPC server-streaming, receives task assignments, dispatches them
//! through a configurable Executor, and reports results back via unary RPC.
//!
//! Implements D-04 (YAML config), D-11 (proxy model), D-14 (separate unary RPC for results),
//! D-15 (executor trait dispatch), D-16 (reconnection with exponential backoff),
//! D-21 (SIGTERM graceful drain).

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use clap::Parser;
use tonic::transport::{Certificate, ClientTlsConfig};
use tracing_subscriber::EnvFilter;
use url::Url;

use xgent_gateway::agent::cli_executor::CliExecutor;
use xgent_gateway::agent::async_api_executor::AsyncApiExecutor;
use xgent_gateway::agent::http_common::find_prefixed_placeholders;
use xgent_gateway::agent::placeholder::resolve_placeholders;
use xgent_gateway::agent::sync_api_executor::SyncApiExecutor;
use xgent_gateway::agent::config::{load_config, AgentConfig, ExecutionMode};
use xgent_gateway::agent::executor::Executor;
use xgent_proto::node_service_client::NodeServiceClient;
use xgent_proto::{DrainNodeRequest, PollTasksRequest, ReportResultRequest};

/// Global flag set when SIGTERM is received to prevent reconnection after graceful shutdown.
static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

#[derive(Parser, Debug)]
#[command(name = "xgent-agent", about = "Node-side runner agent for xgent gateway")]
struct Cli {
    /// Path to agent.yaml config file
    #[arg(long, default_value = "agent.yaml")]
    config: String,

    /// Validate config and print resolved templates without executing
    #[arg(long)]
    dry_run: bool,
}

/// Wait for a shutdown signal (SIGTERM on Unix, Ctrl+C elsewhere).
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("failed to install SIGTERM handler");
        sigterm.recv().await;
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // Load YAML config
    let config = match load_config(&cli.config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to load config '{}': {}", cli.config, e);
            std::process::exit(1);
        }
    };

    // Dry-run: print config summary, validate, preview templates, and exit
    if cli.dry_run {
        println!("Config loaded successfully from '{}'", cli.config);
        println!("  Service:  {}", config.service.name);
        println!("  Mode:     {:?}", config.service.mode);
        println!("  Gateway:  {}", config.gateway.address);
        println!("  Node ID:  {}", config.gateway.node_id);

        let mut errors: Vec<String> = Vec::new();

        // Mode-specific config display and validation
        match config.service.mode {
            ExecutionMode::Cli => {
                if let Some(ref cli_section) = config.cli {
                    println!("  Command:  {:?}", cli_section.command);
                    // Validate command binary exists and is executable
                    if let Some(cmd_path) = cli_section.command.first() {
                        let path = Path::new(cmd_path);
                        if !path.exists() {
                            println!("  Command check: {} ... NOT FOUND", cmd_path);
                            errors.push(format!("command not found: {}", cmd_path));
                        } else {
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                match std::fs::metadata(cmd_path) {
                                    Ok(meta) if meta.permissions().mode() & 0o111 != 0 => {
                                        println!("  Command check: {} ... ok", cmd_path);
                                    }
                                    Ok(_) => {
                                        println!(
                                            "  Command check: {} ... NOT EXECUTABLE",
                                            cmd_path
                                        );
                                        errors.push(format!(
                                            "command not executable: {}",
                                            cmd_path
                                        ));
                                    }
                                    Err(_) => {
                                        println!("  Command check: {} ... NOT FOUND", cmd_path);
                                        errors.push(format!("command not found: {}", cmd_path));
                                    }
                                }
                            }
                            #[cfg(not(unix))]
                            {
                                println!("  Command check: {} ... ok", cmd_path);
                            }
                        }
                    }
                }
            }
            ExecutionMode::SyncApi => {
                if let Some(ref sync_api_section) = config.sync_api {
                    println!("  URL:      {}", sync_api_section.url);
                    println!("  Method:   {}", sync_api_section.method);
                    println!("  Timeout:  {}s", sync_api_section.timeout_secs);
                    // Validate URL
                    match Url::parse(&sync_api_section.url) {
                        Ok(_) => println!("  URL check: {} ... ok", sync_api_section.url),
                        Err(err) => {
                            println!(
                                "  URL check: {} ... INVALID ({})",
                                sync_api_section.url, err
                            );
                            errors.push(format!(
                                "invalid sync-api URL: {} ({})",
                                sync_api_section.url, err
                            ));
                        }
                    }
                }
            }
            ExecutionMode::AsyncApi => {
                if let Some(ref async_api_section) = config.async_api {
                    println!("  Submit URL: {}", async_api_section.submit.url);
                    println!("  Submit Method: {}", async_api_section.submit.method);
                    println!("  Poll URL: {}", async_api_section.poll.url);
                    println!("  Poll Method: {}", async_api_section.poll.method);
                    println!(
                        "  Poll Interval: {}s",
                        async_api_section.poll.interval_secs
                    );
                    println!("  Timeout: {}s", async_api_section.timeout_secs);

                    // Validate submit URL
                    match Url::parse(&async_api_section.submit.url) {
                        Ok(_) => println!(
                            "  Submit URL check: {} ... ok",
                            async_api_section.submit.url
                        ),
                        Err(err) => {
                            println!(
                                "  Submit URL check: {} ... INVALID ({})",
                                async_api_section.submit.url, err
                            );
                            errors.push(format!(
                                "invalid async-api submit URL: {} ({})",
                                async_api_section.submit.url, err
                            ));
                        }
                    }

                    // Validate poll URL (skip if it contains submit_response placeholders)
                    if async_api_section.poll.url.contains("<submit_response.") {
                        println!(
                            "  Poll URL check: {} ... skipped (contains submit_response placeholders)",
                            async_api_section.poll.url
                        );
                    } else {
                        match Url::parse(&async_api_section.poll.url) {
                            Ok(_) => println!(
                                "  Poll URL check: {} ... ok",
                                async_api_section.poll.url
                            ),
                            Err(err) => {
                                println!(
                                    "  Poll URL check: {} ... INVALID ({})",
                                    async_api_section.poll.url, err
                                );
                                errors.push(format!(
                                    "invalid async-api poll URL: {} ({})",
                                    async_api_section.poll.url, err
                                ));
                            }
                        }
                    }
                }
            }
        }

        // Response template preview
        let success_body = &config.response.success.body;
        let mut sample_vars: HashMap<String, String> = HashMap::new();
        sample_vars.insert("payload".to_string(), "(sample payload)".to_string());
        sample_vars.insert(
            "service_name".to_string(),
            config.service.name.clone(),
        );

        match config.service.mode {
            ExecutionMode::Cli => {
                sample_vars
                    .insert("stdout".to_string(), "(sample stdout)".to_string());
                sample_vars
                    .insert("stderr".to_string(), "(sample stderr)".to_string());
            }
            ExecutionMode::SyncApi => {
                sample_vars.insert(
                    "response.result".to_string(),
                    "(sample response.result)".to_string(),
                );
                sample_vars.insert(
                    "response.data".to_string(),
                    "(sample response.data)".to_string(),
                );
                for path in find_prefixed_placeholders(success_body, "response") {
                    let key = format!("response.{}", path);
                    sample_vars
                        .entry(key.clone())
                        .or_insert_with(|| format!("(sample {})", key));
                }
            }
            ExecutionMode::AsyncApi => {
                for path in find_prefixed_placeholders(success_body, "poll_response") {
                    let key = format!("poll_response.{}", path);
                    sample_vars
                        .entry(key.clone())
                        .or_insert_with(|| format!("(sample {})", key));
                }
            }
        }

        println!("  Response template preview:");
        match resolve_placeholders(success_body, &sample_vars) {
            Ok(resolved) => println!("    Success: {}", resolved),
            Err(err) => println!("    Success: (unresolvable: {})", err),
        }

        if let Some(ref failed) = config.response.failed {
            let mut failed_vars = sample_vars.clone();
            // Add stderr for CLI failed templates
            if config.service.mode == ExecutionMode::Cli {
                failed_vars
                    .entry("stderr".to_string())
                    .or_insert_with(|| "(sample stderr)".to_string());
            }
            match resolve_placeholders(&failed.body, &failed_vars) {
                Ok(resolved) => println!("    Failed:  {}", resolved),
                Err(err) => println!("    Failed:  (unresolvable: {})", err),
            }
        }

        // Summary
        if errors.is_empty() {
            println!();
            println!("  \u{2713} Config is valid");
        } else {
            for err in &errors {
                println!("  ERROR: {}", err);
            }
            println!();
            println!("  \u{2717} Config has errors");
            std::process::exit(1);
        }
        return Ok(());
    }

    // Build executor based on execution mode
    let executor: Box<dyn Executor> = match config.service.mode {
        ExecutionMode::Cli => {
            let cli_section = config
                .cli
                .clone()
                .expect("cli section required for cli mode");
            Box::new(CliExecutor::new(
                config.service.name.clone(),
                cli_section,
                config.response.clone(),
            ))
        }
        ExecutionMode::SyncApi => {
            let sync_api_section = config
                .sync_api
                .clone()
                .expect("sync_api section required for sync-api mode");
            match SyncApiExecutor::new(
                config.service.name.clone(),
                sync_api_section,
                config.response.clone(),
                config.debug.dump_request_body,
            ) {
                Ok(executor) => Box::new(executor),
                Err(e) => {
                    eprintln!("failed to initialize sync-api executor: {}", e);
                    std::process::exit(1);
                }
            }
        }
        ExecutionMode::AsyncApi => {
            let async_api_section = config
                .async_api
                .clone()
                .expect("async_api section required for async-api mode");
            match AsyncApiExecutor::new(
                config.service.name.clone(),
                async_api_section,
                config.response.clone(),
                config.debug.dump_request_body,
                config.debug.dump_submit_response,
                config.debug.dump_poll_response,
            ) {
                Ok(executor) => Box::new(executor),
                Err(e) => {
                    eprintln!("failed to initialize async-api executor: {}", e);
                    std::process::exit(1);
                }
            }
        }
    };

    tracing::info!(
        service = %config.service.name,
        node_id = %config.gateway.node_id,
        gateway = %config.gateway.address,
        mode = ?config.service.mode,
        tls = config.gateway.ca_cert.is_some(),
        "agent starting"
    );

    let mut reconnect_delay = Duration::from_secs(1);
    let max_delay = Duration::from_secs(config.gateway.max_reconnect_delay_secs);

    loop {
        match run_poll_loop(&config, executor.as_ref()).await {
            Ok(()) => {
                // If SIGTERM was received, exit cleanly instead of reconnecting
                if SHUTTING_DOWN.load(Ordering::SeqCst) {
                    tracing::info!("agent exiting after graceful shutdown");
                    break;
                }
                tracing::info!("stream ended cleanly, reconnecting");
                reconnect_delay = Duration::from_secs(1);
            }
            Err(e) => {
                if SHUTTING_DOWN.load(Ordering::SeqCst) {
                    tracing::info!("agent exiting after graceful shutdown");
                    break;
                }
                let msg = classify_reconnect_error(&e);
                tracing::warn!(delay=?reconnect_delay, "{}", msg);
                tokio::time::sleep(reconnect_delay).await;
                reconnect_delay = (reconnect_delay * 2).min(max_delay);
            }
        }
    }

    Ok(())
}

/// Classify a reconnect error into a short, human-friendly message.
fn classify_reconnect_error(e: &Box<dyn std::error::Error>) -> String {
    let text = format!("{:#}", e);
    if text.contains("Connection refused") {
        "gateway unavailable (connection refused), retrying".to_string()
    } else if text.contains("broken pipe") || text.contains("stream closed") || text.contains("h2 protocol error") {
        "gateway connection lost (server went away), reconnecting".to_string()
    } else if text.contains("timed out") || text.contains("Timeout") {
        "gateway connection timed out, retrying".to_string()
    } else if text.contains("dns") || text.contains("resolve") {
        "gateway hostname could not be resolved, retrying".to_string()
    } else {
        format!("connection error: {text}, reconnecting")
    }
}

/// Perform the graceful drain sequence: call DrainNode RPC, wait for in-flight task.
async fn graceful_drain(
    drain_client: &mut NodeServiceClient<tonic::transport::Channel>,
    service_name: &str,
    node_id: &str,
    token: &str,
    has_in_flight: bool,
    in_flight_done: &tokio::sync::Notify,
) {
    // Call DrainNode RPC to notify gateway
    let mut drain_req = tonic::Request::new(DrainNodeRequest {
        service_name: service_name.to_string(),
        node_id: node_id.to_string(),
    });
    drain_req.metadata_mut().insert(
        "authorization",
        format!("Bearer {}", token).parse().unwrap(),
    );
    drain_req.metadata_mut().insert(
        "x-service-name",
        service_name.parse().unwrap(),
    );
    match drain_client.drain_node(drain_req).await {
        Ok(resp) => {
            let inner = resp.into_inner();
            tracing::info!(
                drain_timeout_secs = inner.drain_timeout_secs,
                "drain acknowledged by gateway"
            );
        }
        Err(e) => {
            tracing::warn!(error = %e, "drain RPC failed, exiting anyway");
        }
    }

    // Wait for in-flight task to complete (if any)
    if has_in_flight {
        tracing::info!("waiting for in-flight task to complete");
        let timeout = Duration::from_secs(60);
        match tokio::time::timeout(timeout, in_flight_done.notified()).await {
            Ok(()) => tracing::info!("in-flight task completed"),
            Err(_) => tracing::warn!("drain timeout expired, exiting with in-flight task"),
        }
    }

    tracing::info!("graceful shutdown complete");
}

/// Connect to the gateway and process tasks from the server-streaming PollTasks RPC.
/// Returns Ok(()) on stream end or SIGTERM-triggered graceful shutdown.
async fn run_poll_loop(
    config: &AgentConfig,
    executor: &dyn Executor,
) -> Result<(), Box<dyn std::error::Error>> {
    // Build the gRPC channel with optional TLS
    let channel = if config.gateway.ca_cert.is_some() || config.gateway.tls_skip_verify {
        // TLS mode
        let mut tls_config = ClientTlsConfig::new().domain_name("localhost");
        if let Some(ref ca_path) = config.gateway.ca_cert {
            let ca_pem = std::fs::read_to_string(ca_path)?;
            tls_config = tls_config.ca_certificate(Certificate::from_pem(&ca_pem));
        }

        tonic::transport::Channel::from_shared(format!("https://{}", config.gateway.address))?
            .tls_config(tls_config)?
            .connect()
            .await?
    } else {
        // Plain gRPC (dev mode, no TLS)
        tonic::transport::Channel::from_shared(format!("http://{}", config.gateway.address))?
            .connect()
            .await?
    };

    let mut client = NodeServiceClient::new(channel.clone());
    tracing::info!("connected to gateway");

    // Clone the client for result reporting and drain RPC -- tonic clients are
    // clone-safe and reuse the underlying HTTP/2 connection (D-15).
    let report_client = client.clone();
    let mut drain_client = client.clone();

    // Build the PollTasks request with auth token in metadata
    let mut request = tonic::Request::new(PollTasksRequest {
        service_name: config.service.name.clone(),
        node_id: config.gateway.node_id.clone(),
    });
    request.metadata_mut().insert(
        "authorization",
        format!("Bearer {}", config.gateway.token).parse().unwrap(),
    );
    request.metadata_mut().insert(
        "x-service-name",
        config.service.name.parse().unwrap(),
    );

    let mut stream = client.poll_tasks(request).await?.into_inner();

    // Track in-flight task completion
    let in_flight_done = std::sync::Arc::new(tokio::sync::Notify::new());

    // Create the shutdown signal future
    let shutdown = shutdown_signal();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            // Shutdown signal received -- initiate graceful drain
            _ = &mut shutdown => {
                tracing::info!("shutdown signal received, initiating graceful drain");
                SHUTTING_DOWN.store(true, Ordering::SeqCst);

                // In single-threaded select, shutdown can only fire between task
                // executions (while awaiting stream.message), so no task is ever
                // in-flight when drain begins.
                graceful_drain(
                    &mut drain_client,
                    &config.service.name,
                    &config.gateway.node_id,
                    &config.gateway.token,
                    false,
                    &in_flight_done,
                ).await;

                return Ok(());
            }

            // Normal task processing
            msg = stream.message() => {
                match msg? {
                    Some(assignment) => {
                        tracing::info!(task_id = %assignment.task_id, "received task");

                        let exec_result = executor.execute(&assignment).await;

                        if exec_result.success {
                            tracing::info!(task_id = %assignment.task_id, "task completed successfully");
                        } else {
                            tracing::warn!(
                                task_id = %assignment.task_id,
                                error = %exec_result.error_message,
                                "task execution failed"
                            );
                        }

                        let mut report = tonic::Request::new(ReportResultRequest {
                            task_id: assignment.task_id.clone(),
                            success: exec_result.success,
                            result: exec_result.result,
                            error_message: exec_result.error_message,
                            node_id: config.gateway.node_id.clone(),
                            service_name: config.service.name.clone(),
                        });
                        report.metadata_mut().insert(
                            "authorization",
                            format!("Bearer {}", config.gateway.token).parse().unwrap(),
                        );
                        report.metadata_mut().insert(
                            "x-service-name",
                            config.service.name.parse().unwrap(),
                        );

                        let mut rc = report_client.clone();
                        let ack = rc.report_result(report).await?;
                        tracing::info!(
                            task_id = %assignment.task_id,
                            acknowledged = %ack.into_inner().acknowledged,
                            "result reported"
                        );

                        in_flight_done.notify_one();
                    }
                    None => {
                        tracing::info!("stream ended");
                        return Ok(());
                    }
                }
            }
        }
    }
}
