use std::sync::Arc;

use clap::Parser;
use xgent_gateway::{config, grpc, http, queue, state};

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

    // Build shared state
    let state = Arc::new(state::AppState::new(queue, config.clone(), auth_conn));

    let mut handles: Vec<tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>> = Vec::new();

    // gRPC listener (D-07: separate tokio::spawn)
    if config.grpc.enabled {
        let grpc_state = state.clone();
        let grpc_addr = config.grpc.listen_addr.parse()?;
        handles.push(tokio::spawn(async move {
            tracing::info!(%grpc_addr, "gRPC server starting");
            tonic::transport::Server::builder()
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
        handles.push(tokio::spawn(async move {
            let app = axum::Router::new()
                .route("/v1/tasks", axum::routing::post(http::submit::submit_task))
                .route(
                    "/v1/tasks/{task_id}",
                    axum::routing::get(http::result::get_task),
                )
                .with_state(http_state);

            tracing::info!(%http_addr, "HTTP server starting");
            let listener = tokio::net::TcpListener::bind(&http_addr).await?;
            axum::serve(listener, app)
                .await
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })
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
