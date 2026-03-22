//! Sample service for xgent-gateway end-to-end testing.
//!
//! Echoes the task payload back as the result. Optionally simulates processing
//! delay when the `X-Meta-simulate_delay_ms` header is present (per D-01:
//! metadata key `simulate_delay_ms` is forwarded by the runner agent as
//! an HTTP header with `X-Meta-` prefix).
//!
//! Usage:
//!   cargo run -p xgent-gateway --example sample_service
//!   cargo run -p xgent-gateway --example sample_service -- --port 9090
//!
//! Pair with the runner agent:
//!   cargo run -p xgent-gateway --bin xgent-agent -- \
//!     --service-name my-service --token <token> --dispatch-url http://localhost:8090/execute

use std::convert::Infallible;
use std::net::SocketAddr;

use bytes::Bytes;
use clap::Parser;
use http_body_util::{BodyExt, Full};
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use tokio::net::TcpListener;

#[derive(Parser, Debug)]
#[command(name = "sample-service", about = "Echo service for xgent end-to-end testing")]
struct Cli {
    /// Port to listen on (default matches agent's default --dispatch-url)
    #[arg(long, default_value = "8090")]
    port: u16,

    /// Bind address
    #[arg(long, default_value = "0.0.0.0")]
    bind: String,
}

async fn handle(req: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    // Only accept POST /execute
    if req.method() != Method::POST || req.uri().path() != "/execute" {
        return Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Full::new(Bytes::from("not found")))
            .unwrap());
    }

    // Extract X-Task-Id for logging
    let task_id = req
        .headers()
        .get("X-Task-Id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    // Extract optional simulated delay from X-Meta-simulate_delay_ms header
    let delay_ms = req
        .headers()
        .get("X-Meta-simulate_delay_ms")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());

    println!("[task_id={task_id}] POST /execute received");

    if let Some(ms) = delay_ms {
        println!("[task_id={task_id}] simulating {ms}ms delay");
        tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
    }

    // Collect body and echo it back
    let body = req.collect().await.unwrap().to_bytes();
    println!(
        "[task_id={task_id}] echoing {} bytes",
        body.len()
    );

    Ok(Response::new(Full::new(body)))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let addr: SocketAddr = format!("{}:{}", cli.bind, cli.port).parse()?;
    let listener = TcpListener::bind(addr).await?;

    println!("sample-service listening on {}:{}", cli.bind, cli.port);
    println!("  POST /execute -- echo payload (with optional delay)");

    loop {
        let (stream, remote) = listener.accept().await?;
        let io = TokioIo::new(stream);

        tokio::spawn(async move {
            let builder = Builder::new(TokioExecutor::new());
            if let Err(e) = builder.serve_connection(io, service_fn(handle)).await {
                eprintln!("[{remote}] connection error: {e}");
            }
        });
    }
}
