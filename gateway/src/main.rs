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

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(clap::Subcommand, Debug)]
enum Command {
    /// Generate an Argon2id password hash for gateway.toml admin.password_hash
    HashPassword,
}

/// Initialize the tracing subscriber based on logging config.
/// Returns an optional WorkerGuard that must be held for the lifetime of the process
/// to ensure file logging flushes properly.
fn init_tracing(config: &LoggingConfig) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    let is_json = config.format == "json";

    // File layer is always JSON format (structured logs for log aggregation).
    // Constructed once and shared across both stdout format branches.
    let (file_layer, guard) = if let Some(ref file_path) = config.file {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)
            .expect("Failed to open log file");
        let (non_blocking, guard) = tracing_appender::non_blocking(file);
        let layer = tracing_subscriber::fmt::layer()
            .json()
            .with_target(true)
            .with_writer(non_blocking);
        (Some(layer), Some(guard))
    } else {
        (None, None)
    };

    // Stdout layer varies by format; two branches needed because .json() changes the type.
    // Both branches consume the shared file_layer and env_filter.
    if is_json {
        let stdout_layer = tracing_subscriber::fmt::layer()
            .json()
            .with_target(true);
        tracing_subscriber::registry()
            .with(env_filter)
            .with(file_layer)
            .with(stdout_layer)
            .init();
    } else {
        let stdout_layer = tracing_subscriber::fmt::layer()
            .with_target(true);
        tracing_subscriber::registry()
            .with(env_filter)
            .with(file_layer)
            .with(stdout_layer)
            .init();
    }

    guard
}

fn hash_password_interactive() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use argon2::Argon2;
    use password_hash::{PasswordHasher, SaltString};
    use base64::Engine;

    eprint!("Password: ");
    let mut password = String::new();
    std::io::stdin().read_line(&mut password)?;
    let password = password.trim_end();

    if password.is_empty() {
        eprintln!("Error: password cannot be empty");
        std::process::exit(1);
    }

    // Generate a 16-byte random salt, encode as b64 without padding (PHC salt format)
    let mut salt_bytes = [0u8; 16];
    rand::Fill::fill(&mut salt_bytes, &mut rand::rng());
    let salt_b64 = base64::engine::general_purpose::STANDARD_NO_PAD.encode(salt_bytes);
    let salt = SaltString::from_b64(&salt_b64)
        .map_err(|e| format!("salt encoding failed: {e}"))?;

    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| format!("hashing failed: {e}"))?;

    println!("{hash}");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = Cli::parse();

    if let Some(Command::HashPassword) = cli.command {
        return hash_password_interactive();
    }

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
    let metrics_history = Arc::new(std::sync::Mutex::new(
        xgent_gateway::metrics_history::MetricsHistory::new(),
    ));
    let state = Arc::new(state::AppState::new(queue, config.clone(), auth_conn, http_client, metrics, metrics_history));

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

    // Spawn background metrics snapshot capture (10s interval, per D-03)
    let snapshot_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        interval.tick().await; // skip first immediate tick

        loop {
            interval.tick().await;
            // Refresh gauges first to ensure fresh values (per pitfall 5 from research)
            if let Err(e) = xgent_gateway::metrics::refresh_gauges(&snapshot_state).await {
                tracing::warn!(error = %e, "gauge refresh before snapshot failed");
                continue;
            }
            let snapshot = xgent_gateway::metrics_history::capture_snapshot(&snapshot_state.metrics);
            if let Ok(mut history) = snapshot_state.metrics_history.lock() {
                history.push_snapshot(snapshot);
            }
        }
    });
    tracing::info!("background metrics snapshot started (10s interval)");

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

            // Auth routes -- no authentication required (login must be unauthenticated)
            let auth_routes = axum::Router::new()
                .route(
                    "/v1/admin/auth/login",
                    axum::routing::post(http::auth::login),
                )
                .route(
                    "/v1/admin/auth/logout",
                    axum::routing::post(http::auth::logout),
                )
                .route(
                    "/v1/admin/auth/refresh",
                    axum::routing::post(http::auth::refresh),
                );

            // Admin routes -- protected by session cookie if configured
            let admin_routes = axum::Router::new()
                .route(
                    "/v1/admin/api-keys",
                    axum::routing::post(http::admin::create_api_key)
                        .get(http::admin::list_api_keys),
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
                    axum::routing::post(http::admin::create_node_token)
                        .get(http::admin::list_node_tokens),
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
                    "/v1/admin/tasks",
                    axum::routing::get(http::admin::list_tasks_handler),
                )
                .route(
                    "/v1/admin/tasks/{task_id}",
                    axum::routing::get(http::admin::get_task_detail_handler),
                )
                .route(
                    "/v1/admin/tasks/{task_id}/cancel",
                    axum::routing::post(http::admin::cancel_task_handler),
                )
                .route(
                    "/metrics",
                    axum::routing::get(http::admin::metrics_handler),
                )
                .route(
                    "/v1/admin/metrics/summary",
                    axum::routing::get(http::admin::metrics_summary_handler),
                )
                .route(
                    "/v1/admin/metrics/history",
                    axum::routing::get(http::admin::metrics_history_handler),
                )
                .layer(axum::middleware::from_fn_with_state(
                    http_state.clone(),
                    http::auth::session_auth_middleware,
                ));

            // CORS layer -- must be outermost (outside auth middleware)
            let cors = if let Some(ref origin) = http_state.config.admin.cors_origin {
                use tower_http::cors::{CorsLayer, AllowOrigin};
                use axum::http::{Method, header::{CONTENT_TYPE, COOKIE}};
                CorsLayer::new()
                    .allow_origin(AllowOrigin::exact(
                        origin.parse().expect("invalid cors_origin"),
                    ))
                    .allow_methods([
                        Method::GET,
                        Method::POST,
                        Method::PATCH,
                        Method::DELETE,
                        Method::OPTIONS,
                    ])
                    .allow_headers([CONTENT_TYPE, COOKIE])
                    .allow_credentials(true)
            } else {
                // Dev mode: mirror the request Origin so credentials work
                use tower_http::cors::{CorsLayer, AllowOrigin};
                use axum::http::{Method, header::{CONTENT_TYPE, COOKIE}};
                CorsLayer::new()
                    .allow_origin(AllowOrigin::mirror_request())
                    .allow_methods([
                        Method::GET,
                        Method::POST,
                        Method::PATCH,
                        Method::DELETE,
                        Method::OPTIONS,
                    ])
                    .allow_headers([CONTENT_TYPE, COOKIE])
                    .allow_credentials(true)
            };

            let app = axum::Router::new()
                .merge(api_routes)
                .merge(auth_routes)
                .merge(admin_routes)
                .layer(cors)
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
            Ok(Err(e)) => return Err(e),
            Err(e) => return Err(e.into()),
        }
    }

    Ok(())
}
