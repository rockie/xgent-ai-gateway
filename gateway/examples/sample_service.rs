//! Sample service for xgent-gateway end-to-end testing.
//!
//! Echoes the task payload back as the result. Optionally simulates processing
//! delay when the `X-Meta-simulate_delay_ms` header is present (per D-01:
//! metadata key `simulate_delay_ms` is forwarded by the runner agent as
//! an HTTP header with `X-Meta-` prefix).
//!
//! Endpoints:
//!   POST /execute          -- echo payload (with optional delay)
//!   POST /sync             -- echo payload in JSON wrapper
//!   POST /async/submit     -- create async job
//!   GET  /async/status/:id -- poll job status (completes after 3 polls)
//!
//! Usage:
//!   cargo run -p xgent-gateway --example sample_service
//!   cargo run -p xgent-gateway --example sample_service -- --port 9090

use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Mutex as StdMutex;
use std::sync::Arc;
use std::time::Instant;

use bytes::Bytes;
use clap::Parser;
use http_body_util::{BodyExt, Full};
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use tokio::net::TcpListener;
use uuid::Uuid;

/// In-memory async job state.
struct AsyncJob {
    payload: String,
    #[allow(dead_code)]
    created: Instant,
    polls: u32,
}

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

type Jobs = Arc<StdMutex<HashMap<String, AsyncJob>>>;

async fn handle(
    req: Request<hyper::body::Incoming>,
    jobs: Jobs,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    // Route: POST /execute -- original echo endpoint
    if method == Method::POST && path == "/execute" {
        return handle_execute(req).await;
    }

    // Route: POST /sync -- echo in JSON wrapper
    if method == Method::POST && path == "/sync" {
        return handle_sync(req).await;
    }

    // Route: POST /async/submit -- create async job
    if method == Method::POST && path == "/async/submit" {
        return handle_async_submit(req, jobs).await;
    }

    // Route: GET /async/status/:id -- poll job status
    if method == Method::GET && path.starts_with("/async/status/") {
        let id = path.strip_prefix("/async/status/").unwrap_or("");
        if !id.is_empty() {
            return handle_async_status(req, jobs, id.to_string()).await;
        }
    }

    Ok(Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Full::new(Bytes::from("not found")))
        .unwrap())
}

/// POST /execute -- echo payload back (with optional delay).
async fn handle_execute(
    req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let task_id = req
        .headers()
        .get("X-Task-Id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

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

    let body = req.collect().await.unwrap().to_bytes();
    println!("[task_id={task_id}] echoing {} bytes", body.len());

    Ok(Response::new(Full::new(body)))
}

/// POST /sync -- echo payload in JSON wrapper.
async fn handle_sync(
    req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let task_id = req
        .headers()
        .get("X-Task-Id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    println!("[task_id={task_id}] POST /sync received");

    let body = req.collect().await.unwrap().to_bytes();
    let text = String::from_utf8_lossy(&body);
    let len = body.len();

    let json = format!(
        r#"{{"status":"ok","result":{{"text":"{}","length":{}}}}}"#,
        text.replace('\\', "\\\\").replace('"', "\\\""),
        len
    );

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(json)))
        .unwrap())
}

/// POST /async/submit -- create an async job.
async fn handle_async_submit(
    req: Request<hyper::body::Incoming>,
    jobs: Jobs,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let body = req.collect().await.unwrap().to_bytes();
    let payload = String::from_utf8_lossy(&body).to_string();
    let job_id = Uuid::now_v7().to_string();

    println!("[job_id={job_id}] POST /async/submit received");

    {
        let mut map = jobs.lock().unwrap();
        map.insert(
            job_id.clone(),
            AsyncJob {
                payload,
                created: Instant::now(),
                polls: 0,
            },
        );
    }

    let json = format!(r#"{{"job_id":"{}","status":"accepted"}}"#, job_id);

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(json)))
        .unwrap())
}

/// GET /async/status/:id -- poll job status.
async fn handle_async_status(
    _req: Request<hyper::body::Incoming>,
    jobs: Jobs,
    id: String,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let mut map = jobs.lock().unwrap();

    let Some(job) = map.get_mut(&id) else {
        let json = format!(r#"{{"error":"job not found"}}"#);
        return Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header("Content-Type", "application/json")
            .body(Full::new(Bytes::from(json)))
            .unwrap());
    };

    job.polls += 1;
    let polls = job.polls;

    println!("[job_id={id}] GET /async/status (poll #{polls})");

    let json = if polls >= 3 {
        let text = job.payload.replace('\\', "\\\\").replace('"', "\\\"");
        format!(
            r#"{{"job_id":"{}","status":"completed","result":{{"text":"{}","processed":true}}}}"#,
            id, text
        )
    } else {
        let progress = polls * 33;
        format!(
            r#"{{"job_id":"{}","status":"processing","progress":{}}}"#,
            id, progress
        )
    };

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(json)))
        .unwrap())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let addr: SocketAddr = format!("{}:{}", cli.bind, cli.port).parse()?;
    let listener = TcpListener::bind(addr).await?;
    let jobs: Jobs = Arc::new(StdMutex::new(HashMap::new()));

    println!("sample-service listening on {}:{}", cli.bind, cli.port);
    println!("  POST /execute            -- echo payload (with optional delay)");
    println!("  POST /sync               -- echo payload in JSON wrapper");
    println!("  POST /async/submit       -- create async job");
    println!("  GET  /async/status/:id   -- poll job status (completes after 3 polls)");

    loop {
        let (stream, remote) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let state = Arc::clone(&jobs);

        tokio::spawn(async move {
            let builder = Builder::new(TokioExecutor::new());
            if let Err(e) = builder
                .serve_connection(
                    io,
                    service_fn(move |req| {
                        let st = Arc::clone(&state);
                        async move { handle(req, st).await }
                    }),
                )
                .await
            {
                eprintln!("[{remote}] connection error: {e}");
            }
        });
    }
}
