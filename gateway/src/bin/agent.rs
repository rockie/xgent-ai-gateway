//! xgent-agent: Lightweight runner agent (node-side proxy) that connects to the
//! gateway via gRPC server-streaming, receives task assignments, dispatches them
//! to a local HTTP service, and reports results back via unary RPC.
//!
//! Implements D-11 (proxy model), D-14 (separate unary RPC for results),
//! D-16 (reconnection with exponential backoff), D-21 (SIGTERM graceful drain).

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use clap::Parser;
use tonic::transport::{Certificate, ClientTlsConfig};
use tracing_subscriber::EnvFilter;

use xgent_proto::node_service_client::NodeServiceClient;
use xgent_proto::{DrainNodeRequest, PollTasksRequest, ReportResultRequest, TaskAssignment};

/// Global flag set when SIGTERM is received to prevent reconnection after graceful shutdown.
static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

#[derive(Parser, Debug)]
#[command(name = "xgent-agent", about = "Node-side runner agent for xgent gateway")]
struct Cli {
    /// Gateway gRPC address (host:port, e.g., localhost:50051)
    #[arg(long, env = "AGENT_GATEWAY_ADDR", default_value = "localhost:50051")]
    gateway_addr: String,

    /// Service name this agent serves
    #[arg(long, env = "AGENT_SERVICE_NAME")]
    service_name: String,

    /// Unique node ID for this agent
    #[arg(long, env = "AGENT_NODE_ID", default_value_t = uuid::Uuid::now_v7().to_string())]
    node_id: String,

    /// Local service URL to dispatch tasks to
    #[arg(long, env = "AGENT_DISPATCH_URL", default_value = "http://localhost:8090/execute")]
    dispatch_url: String,

    /// Node authentication token (required for gateway communication)
    #[arg(long, env = "XGENT_NODE_TOKEN")]
    token: String,

    /// Path to CA certificate for TLS verification
    #[arg(long, env = "XGENT_CA_CERT")]
    ca_cert: Option<String>,

    /// Skip TLS verification (development only -- NOT for production)
    #[arg(long, default_value = "false")]
    tls_skip_verify: bool,

    /// Max reconnect delay in seconds
    #[arg(long, default_value = "30")]
    max_reconnect_delay_secs: u64,
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

    tracing::info!(
        service = %cli.service_name,
        node_id = %cli.node_id,
        gateway = %cli.gateway_addr,
        dispatch_url = %cli.dispatch_url,
        tls = cli.ca_cert.is_some(),
        "agent starting"
    );

    let http_client = reqwest::Client::new();
    let mut reconnect_delay = Duration::from_secs(1);
    let max_delay = Duration::from_secs(cli.max_reconnect_delay_secs);

    loop {
        match run_poll_loop(&cli, &http_client).await {
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
                tracing::error!(?e, delay=?reconnect_delay, "poll loop error, reconnecting");
                tokio::time::sleep(reconnect_delay).await;
                reconnect_delay = (reconnect_delay * 2).min(max_delay);
            }
        }
    }

    Ok(())
}

/// Perform the graceful drain sequence: call DrainNode RPC, wait for in-flight task.
async fn graceful_drain(
    drain_client: &mut NodeServiceClient<tonic::transport::Channel>,
    service_name: &str,
    node_id: &str,
    has_in_flight: bool,
    in_flight_done: &tokio::sync::Notify,
) {
    // Call DrainNode RPC to notify gateway
    let drain_req = tonic::Request::new(DrainNodeRequest {
        service_name: service_name.to_string(),
        node_id: node_id.to_string(),
    });
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
    cli: &Cli,
    http_client: &reqwest::Client,
) -> Result<(), Box<dyn std::error::Error>> {
    // Build the gRPC channel with optional TLS
    let channel = if cli.ca_cert.is_some() || cli.tls_skip_verify {
        // TLS mode
        let mut tls_config = ClientTlsConfig::new().domain_name("localhost");
        if let Some(ref ca_path) = cli.ca_cert {
            let ca_pem = std::fs::read_to_string(ca_path)?;
            tls_config = tls_config.ca_certificate(Certificate::from_pem(&ca_pem));
        }

        tonic::transport::Channel::from_shared(format!("https://{}", cli.gateway_addr))?
            .tls_config(tls_config)?
            .connect()
            .await?
    } else {
        // Plain gRPC (dev mode, no TLS)
        tonic::transport::Channel::from_shared(format!("http://{}", cli.gateway_addr))?
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
        service_name: cli.service_name.clone(),
        node_id: cli.node_id.clone(),
    });
    request.metadata_mut().insert(
        "authorization",
        format!("Bearer {}", cli.token).parse().unwrap(),
    );

    let mut stream = client.poll_tasks(request).await?.into_inner();

    // Track in-flight task completion
    let in_flight_done = std::sync::Arc::new(tokio::sync::Notify::new());
    let mut has_in_flight = false;

    // Create the shutdown signal future
    let shutdown = shutdown_signal();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            // Shutdown signal received -- initiate graceful drain
            _ = &mut shutdown => {
                tracing::info!("shutdown signal received, initiating graceful drain");
                SHUTTING_DOWN.store(true, Ordering::SeqCst);

                graceful_drain(
                    &mut drain_client,
                    &cli.service_name,
                    &cli.node_id,
                    has_in_flight,
                    &in_flight_done,
                ).await;

                return Ok(());
            }

            // Normal task processing
            msg = stream.message() => {
                match msg? {
                    Some(assignment) => {
                        tracing::info!(task_id = %assignment.task_id, "received task");
                        has_in_flight = true;

                        let dispatch_result = dispatch_task(http_client, &cli.dispatch_url, &assignment).await;

                        let report = match dispatch_result {
                            Ok(result_bytes) => {
                                tracing::info!(task_id = %assignment.task_id, "task completed successfully");
                                ReportResultRequest {
                                    task_id: assignment.task_id.clone(),
                                    success: true,
                                    result: result_bytes,
                                    error_message: String::new(),
                                }
                            }
                            Err(e) => {
                                tracing::warn!(task_id = %assignment.task_id, error = %e, "task dispatch failed");
                                ReportResultRequest {
                                    task_id: assignment.task_id.clone(),
                                    success: false,
                                    result: Vec::new(),
                                    error_message: e.to_string(),
                                }
                            }
                        };

                        let mut rc = report_client.clone();
                        let ack = rc.report_result(report).await?;
                        tracing::info!(
                            task_id = %assignment.task_id,
                            acknowledged = %ack.into_inner().acknowledged,
                            "result reported"
                        );

                        has_in_flight = false;
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

/// Dispatch a task to the local service via HTTP POST.
async fn dispatch_task(
    http_client: &reqwest::Client,
    dispatch_url: &str,
    assignment: &TaskAssignment,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let resp = http_client
        .post(dispatch_url)
        .header("X-Task-Id", &assignment.task_id)
        .body(assignment.payload.clone())
        .send()
        .await?;

    if resp.status().is_success() {
        Ok(resp.bytes().await?.to_vec())
    } else {
        Err(format!("dispatch failed with status {}", resp.status()).into())
    }
}
