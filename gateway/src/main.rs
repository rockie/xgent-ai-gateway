use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use xgent_gateway::{auth, config, grpc, http, queue, state, tls};

use xgent_proto::node_service_server::NodeServiceServer;
use xgent_proto::task_service_server::TaskServiceServer;

#[derive(Parser, Debug)]
#[command(name = "xgent-gateway", about = "Pull-model task gateway")]
struct Cli {
    /// Path to configuration TOML file
    #[arg(long)]
    config: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    let config = config::load_config(cli.config.as_deref())?;
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

    // Build shared state
    let state = Arc::new(state::AppState::new(queue, config.clone(), auth_conn, http_client));

    // Spawn background reaper for timed-out tasks
    let reaper_state = state.clone();
    tokio::spawn(async move {
        xgent_gateway::reaper::run_reaper(reaper_state).await;
    });
    tracing::info!("background reaper started (30s interval)");

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

            grpc_builder
                .add_service(TaskServiceServer::new(
                    grpc::GrpcTaskService::new(grpc_state.clone()),
                ))
                .add_service(NodeServiceServer::new(
                    grpc::GrpcNodeService::new(grpc_state),
                ))
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

            // Admin routes -- unauthenticated in Phase 2 (admin auth deferred to Phase 3)
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
                );

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
                // Plain HTTP mode (backward compatible with Phase 1)
                tracing::info!(%http_addr, "HTTP server starting");
                let listener = tokio::net::TcpListener::bind(&http_addr).await?;
                axum::serve(listener, app)
                    .await
                    .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })
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
