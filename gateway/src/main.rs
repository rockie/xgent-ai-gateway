#[cfg(target_env = "musl")]
use tikv_jemallocator::Jemalloc;

#[cfg(target_env = "musl")]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use tracing_subscriber::prelude::*;
use xgent_gateway::{auth, config, grpc, http, queue, state, tls};
use xgent_gateway::config::LoggingConfig;

use xgent_proto::node_service_server::NodeServiceServer;
use xgent_proto::task_service_server::TaskServiceServer;

#[derive(Parser, Debug)]
#[command(name = "xgent-gateway", about = "Pull-model task gateway")]
struct Cli {
    /// Path to configuration TOML file
    #[arg(long)]
    config: Option<String>,
}

/// Initialize the tracing subscriber based on logging config.
/// Returns an optional WorkerGuard that must be held for the lifetime of the process
/// to ensure file logging flushes properly.
fn init_tracing(config: &LoggingConfig) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    let is_json = config.format == "json";

    match (&config.file, is_json) {
        (Some(file_path), true) => {
            let file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(file_path)
                .expect("Failed to open log file");
            let (non_blocking, guard) = tracing_appender::non_blocking(file);
            let file_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_target(true)
                .with_writer(non_blocking);
            let stdout_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_target(true);
            tracing_subscriber::registry()
                .with(env_filter)
                .with(stdout_layer)
                .with(file_layer)
                .init();
            Some(guard)
        }
        (Some(file_path), false) => {
            let file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(file_path)
                .expect("Failed to open log file");
            let (non_blocking, guard) = tracing_appender::non_blocking(file);
            let file_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_target(true)
                .with_writer(non_blocking);
            let stdout_layer = tracing_subscriber::fmt::layer()
                .with_target(true);
            tracing_subscriber::registry()
                .with(env_filter)
                .with(stdout_layer)
                .with(file_layer)
                .init();
            Some(guard)
        }
        (None, true) => {
            let stdout_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_target(true);
            tracing_subscriber::registry()
                .with(env_filter)
                .with(stdout_layer)
                .init();
            None
        }
        (None, false) => {
            let stdout_layer = tracing_subscriber::fmt::layer()
                .with_target(true);
            tracing_subscriber::registry()
                .with(env_filter)
                .with(stdout_layer)
                .init();
            None
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = Cli::parse();

    let config = config::load_config(cli.config.as_deref())?;
    let _log_guard = init_tracing(&config.logging);
    tracing::info!("xgent-gateway starting");

    // Connect to Redis
    let queue = queue::RedisQueue::new(&config).await?;
    tracing::info!(redis_url=%config.redis.url, "connected to Redis");

    // Open a dedicated Redis connection for auth lookups
    let auth_client = redis::Client::open(config.redis.url.as_str())?;
    let auth_conn = auth_client.get_multiplexed_async_connection().await?;
    tracing::info!("auth Redis connection established");

    // Build HTTP client for callback delivery
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(config.callback.timeout_secs))
        .connect_timeout(std::time::Duration::from_secs(5))
        .pool_max_idle_per_host(10)
        .pool_idle_timeout(std::time::Duration::from_secs(90))
        .build()
        .expect("Failed to build HTTP client for callbacks");

    // Build metrics and shared state
    let metrics = xgent_gateway::metrics::Metrics::new();
    let state = Arc::new(state::AppState::new(queue, config.clone(), auth_conn, http_client, metrics));

    // Spawn background reaper for timed-out tasks
    let reaper_state = state.clone();
    tokio::spawn(async move {
        xgent_gateway::reaper::run_reaper(reaper_state).await;
    });
    tracing::info!("background reaper started (30s interval)");

    // Spawn background gauge refresh for Prometheus metrics (queue_depth, nodes_active)
    let gauge_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(15));
        interval.tick().await; // skip first immediate tick

        loop {
            interval.tick().await;
            if let Err(e) = xgent_gateway::metrics::refresh_gauges(&gauge_state).await {
                tracing::warn!(error = %e, "gauge refresh cycle failed");
            }
        }
    });
    tracing::info!("background gauge refresh started (15s interval)");

    let mut handles: Vec<tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>> = Vec::new();

    // gRPC listener (D-07: separate tokio::spawn)
    if config.grpc.enabled {
        let grpc_state = state.clone();
        let grpc_addr = config.grpc.listen_addr.parse()?;
        let grpc_tls = config.grpc.tls.clone();
        handles.push(tokio::spawn(async move {
            tracing::info!(%grpc_addr, "gRPC server starting");

            let mut grpc_builder = tonic::transport::Server::builder()
                .http2_keepalive_interval(Some(Duration::from_secs(30)))
                .http2_keepalive_timeout(Some(Duration::from_secs(10)));

            if let Some(ref tls_cfg) = grpc_tls {
                let tls_config = tls::config::build_grpc_tls_config(tls_cfg)
                    .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                        format!("gRPC TLS config error: {e}").into()
                    })?;
                grpc_builder = grpc_builder.tls_config(tls_config)
                    .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?;
                tracing::info!("gRPC mTLS enabled");
            }

            let task_svc = TaskServiceServer::new(
                grpc::GrpcTaskService::new(grpc_state.clone()),
            );
            let node_svc = NodeServiceServer::new(
                grpc::GrpcNodeService::new(grpc_state.clone()),
            );

            grpc_builder
                .add_service(grpc::ApiKeyAuthLayer::new(task_svc, grpc_state.clone()))
                .add_service(grpc::NodeTokenAuthLayer::new(node_svc, grpc_state))
                .serve(grpc_addr)
                .await
                .map_err(|e| e.into())
        }));
    }

    // HTTP listener (D-07: separate tokio::spawn)
    if config.http.enabled {
        let http_state = state.clone();
        let http_addr = config.http.listen_addr.clone();
        let http_tls = config.http.tls.clone();
        handles.push(tokio::spawn(async move {
            // API routes protected by API key auth middleware
            let api_routes = axum::Router::new()
                .route("/v1/tasks", axum::routing::post(http::submit::submit_task))
                .route(
                    "/v1/tasks/{task_id}",
                    axum::routing::get(http::result::get_task),
                )
                .layer(axum::middleware::from_fn_with_state(
                    http_state.clone(),
                    auth::api_key::api_key_auth_middleware,
                ));

            // Admin routes -- protected by session cookie if configured
            let admin_routes = axum::Router::new()
                .route(
                    "/v1/admin/api-keys",
                    axum::routing::post(http::admin::create_api_key),
                )
                .route(
                    "/v1/admin/api-keys/revoke",
                    axum::routing::post(http::admin::revoke_api_key),
                )
                .route(
                    "/v1/admin/api-keys/{key_hash}",
                    axum::routing::patch(http::admin::update_api_key_callback),
                )
                .route(
                    "/v1/admin/node-tokens",
                    axum::routing::post(http::admin::create_node_token),
                )
                .route(
                    "/v1/admin/node-tokens/revoke",
                    axum::routing::post(http::admin::revoke_node_token),
                )
                .route(
                    "/v1/admin/services",
                    axum::routing::post(http::admin::register_service)
                        .get(http::admin::list_services),
                )
                .route(
                    "/v1/admin/services/{name}",
                    axum::routing::get(http::admin::get_service_detail)
                        .delete(http::admin::deregister_service),
                )
                .route(
                    "/v1/admin/health",
                    axum::routing::get(http::admin::health_handler),
                )
                .route(
                    "/metrics",
                    axum::routing::get(http::admin::metrics_handler),
                )
                .layer(axum::middleware::from_fn_with_state(
                    http_state.clone(),
                    http::auth::session_auth_middleware,
                ));

            let app = axum::Router::new()
                .merge(api_routes)
                .merge(admin_routes)
                .with_state(http_state);

            if let Some(ref tls_cfg) = http_tls {
                // TLS mode: manual accept loop with rustls
                let tls_config = tls::config::build_http_tls_config(tls_cfg)
                    .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                        format!("HTTP TLS config error: {e}").into()
                    })?;
                let tls_acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(tls_config));
                let tcp_listener = tokio::net::TcpListener::bind(&http_addr).await?;

                tracing::info!(%http_addr, "HTTPS server starting (TLS enabled)");

                loop {
                    let (tcp_stream, addr) = tcp_listener.accept().await?;
                    let acceptor = tls_acceptor.clone();
                    let app = app.clone();
                    tokio::spawn(async move {
                        match acceptor.accept(tcp_stream).await {
                            Ok(tls_stream) => {
                                let io = hyper_util::rt::TokioIo::new(tls_stream);
                                let service = hyper_util::service::TowerToHyperService::new(app);
                                let mut builder = hyper_util::server::conn::auto::Builder::new(
                                    hyper_util::rt::TokioExecutor::new(),
                                );
                                builder
                                    .http2()
                                    .keep_alive_interval(Some(Duration::from_secs(30)))
                                    .keep_alive_timeout(Duration::from_secs(10));
                                let conn = builder.serve_connection(io, service);
                                if let Err(e) = conn.await {
                                    tracing::debug!(%addr, error=%e, "HTTP connection error");
                                }
                            }
                            Err(e) => {
                                tracing::debug!(%addr, error=%e, "TLS handshake failed");
                            }
                        }
                    });
                }
            } else {
                // Plain HTTP mode with keepalive (INFR-06 fix)
                let listener = tokio::net::TcpListener::bind(&http_addr).await?;
                tracing::info!(%http_addr, "HTTP server starting (plain, with keepalive)");
                loop {
                    let (tcp_stream, addr) = listener.accept().await?;
                    let app = app.clone();
                    tokio::spawn(async move {
                        let io = hyper_util::rt::TokioIo::new(tcp_stream);
                        let service = hyper_util::service::TowerToHyperService::new(app);
                        let mut builder = hyper_util::server::conn::auto::Builder::new(
                            hyper_util::rt::TokioExecutor::new(),
                        );
                        builder
                            .http2()
                            .keep_alive_interval(Some(Duration::from_secs(30)))
                            .keep_alive_timeout(Duration::from_secs(10));
                        if let Err(e) = builder.serve_connection(io, service).await {
                            tracing::debug!(%addr, error=%e, "HTTP connection error");
                        }
                    });
                }
            }
        }));
    }

    // Wait for all servers (error if any crashes)
    let results = futures::future::join_all(handles).await;
    for result in results {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(e.into()),
            Err(e) => return Err(e.into()),
        }
    }

    Ok(())
}
