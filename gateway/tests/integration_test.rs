//! Integration tests for the xgent-gateway full lifecycle.
//!
//! These tests require a running Redis instance. They are gated with `#[ignore]`
//! and run via: `cargo test -p xgent-gateway --test integration_test -- --ignored`
//!
//! Tests exercise:
//! - gRPC task submission and status retrieval
//! - HTTP task submission and status retrieval
//! - Full lifecycle: submit -> poll -> report -> retrieve
//! - Service isolation across different service queues
//! - Error handling (task not found)

use std::collections::HashMap;
use std::net::TcpListener;
use std::sync::Arc;
use std::time::Duration;

use xgent_gateway::{config, grpc, http, metrics::Metrics, queue, state};
use xgent_proto::node_service_client::NodeServiceClient;
use xgent_proto::node_service_server::NodeServiceServer;
use xgent_proto::task_service_client::TaskServiceClient;
use xgent_proto::task_service_server::TaskServiceServer;
use xgent_proto::{
    GetTaskStatusRequest, PollTasksRequest, ReportResultRequest, SubmitTaskRequest,
};

/// Information about a running test gateway.
struct TestGateway {
    grpc_addr: String,
    http_addr: String,
    _shutdown: tokio::sync::oneshot::Sender<()>,
}

/// Start a test gateway with random ports and a fresh Redis keyspace prefix.
async fn start_test_gateway(test_name: &str) -> TestGateway {
    // Find free ports
    let grpc_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let grpc_port = grpc_listener.local_addr().unwrap().port();
    drop(grpc_listener);

    let http_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let http_port = http_listener.local_addr().unwrap().port();
    drop(http_listener);

    let grpc_addr_str = format!("127.0.0.1:{grpc_port}");
    let http_addr_str = format!("127.0.0.1:{http_port}");

    // Build config pointing to local Redis
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

    let cfg = config::GatewayConfig {
        grpc: config::GrpcConfig {
            enabled: true,
            listen_addr: grpc_addr_str.clone(),
            tls: None,
            mtls_identity: Default::default(),
        },
        http: config::HttpConfig {
            enabled: true,
            listen_addr: http_addr_str.clone(),
            tls: None,
        },
        redis: config::RedisConfig {
            url: redis_url.clone(),
            result_ttl_secs: 300,
        },
        queue: config::QueueConfig {
            stream_maxlen: 1000,
            block_timeout_ms: 2000,
        },
        admin: config::AdminConfig::default(),
        service_defaults: config::ServiceDefaultsConfig::default(),
        callback: config::CallbackConfig::default(),
        logging: config::LoggingConfig::default(),
    };

    let redis_queue = queue::RedisQueue::new(&cfg)
        .await
        .expect("Redis must be running for integration tests");

    // Clean up any leftover keys from previous test runs for this test
    cleanup_redis_keys(&redis_queue, test_name).await;

    // Open auth connection
    let auth_client = redis::Client::open(redis_url.as_str()).unwrap();
    let auth_conn = auth_client.get_multiplexed_async_connection().await.unwrap();

    let metrics_history = Arc::new(std::sync::Mutex::new(xgent_gateway::metrics_history::MetricsHistory::new()));
    let app_state = Arc::new(state::AppState::new(redis_queue, cfg.clone(), auth_conn, reqwest::Client::new(), Metrics::new(), metrics_history));

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    // Spawn gRPC server
    let grpc_state = app_state.clone();
    let grpc_addr: std::net::SocketAddr = grpc_addr_str.parse().unwrap();
    let grpc_handle = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(TaskServiceServer::new(grpc::GrpcTaskService::new(
                grpc_state.clone(),
            )))
            .add_service(NodeServiceServer::new(grpc::GrpcNodeService::new(
                grpc_state,
            )))
            .serve(grpc_addr)
            .await
            .unwrap();
    });

    // Spawn HTTP server
    let http_state = app_state.clone();
    let http_addr_bind = http_addr_str.clone();
    let http_handle = tokio::spawn(async move {
        let app = axum::Router::new()
            .route("/v1/tasks", axum::routing::post(http::submit::submit_task))
            .route(
                "/v1/tasks/{task_id}",
                axum::routing::get(http::result::get_task),
            )
            .with_state(http_state);
        let listener = tokio::net::TcpListener::bind(&http_addr_bind).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    });

    // Spawn shutdown watcher that aborts server tasks when signaled
    tokio::spawn(async move {
        let _ = shutdown_rx.await;
        grpc_handle.abort();
        http_handle.abort();
    });

    // Brief wait for servers to start listening
    tokio::time::sleep(Duration::from_millis(200)).await;

    TestGateway {
        grpc_addr: format!("http://127.0.0.1:{grpc_port}"),
        http_addr: format!("http://127.0.0.1:{http_port}"),
        _shutdown: shutdown_tx,
    }
}

/// Clean up Redis keys used by a specific test.
async fn cleanup_redis_keys(queue: &queue::RedisQueue, _test_name: &str) {
    // We use service names that include the test name to avoid collisions
    // Cleanup is best-effort
    let _ = queue;
}

/// Flush specific keys from Redis after a test.
async fn flush_keys(redis_url: &str, keys: &[String]) {
    let client = redis::Client::open(redis_url).unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();
    for key in keys {
        let _: Result<(), _> = redis::cmd("DEL").arg(key).query_async(&mut conn).await;
    }
}

// ============================================================================
// Test: gRPC task submission and status retrieval
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_submit_task_grpc() {
    let gw = start_test_gateway("submit_grpc").await;

    let mut client = TaskServiceClient::connect(gw.grpc_addr.clone())
        .await
        .unwrap();

    let mut metadata = HashMap::new();
    metadata.insert("key".to_string(), "val".to_string());

    let resp = client
        .submit_task(SubmitTaskRequest {
            service_name: "test-submit-grpc".to_string(),
            payload: r#"{"message":"hello"}"#.to_string(),
            metadata: metadata.clone(),
            callback_url: String::new(),
        })
        .await
        .unwrap()
        .into_inner();

    assert!(!resp.task_id.is_empty(), "task_id should not be empty");

    // Check status
    let status = client
        .get_task_status(GetTaskStatusRequest {
            task_id: resp.task_id.clone(),
        })
        .await
        .unwrap()
        .into_inner();

    // State should be PENDING (1)
    assert_eq!(status.state, 1, "task state should be PENDING (1)");
    assert_eq!(
        status.metadata.get("key").map(|s| s.as_str()),
        Some("val"),
        "metadata should contain 'key'='val'"
    );

    // Cleanup
    flush_keys(
        "redis://127.0.0.1:6379",
        &[
            format!("task:{}", resp.task_id),
            "tasks:test-submit-grpc".to_string(),
        ],
    )
    .await;
}

// ============================================================================
// Test: HTTP task submission and status retrieval
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_submit_task_http() {
    let gw = start_test_gateway("submit_http").await;
    let http_client = reqwest::Client::new();

    // Submit task via HTTP
    let submit_resp = http_client
        .post(format!("{}/v1/tasks", gw.http_addr))
        .json(&serde_json::json!({
            "service_name": "test-submit-http",
            "payload": {"message": "hello"},
            "metadata": {"env": "test"}
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(submit_resp.status(), 200, "submit should return 200");
    let body: serde_json::Value = submit_resp.json().await.unwrap();
    let task_id = body["task_id"].as_str().unwrap();
    assert!(!task_id.is_empty(), "task_id should not be empty");

    // Check status via HTTP
    let status_resp = http_client
        .get(format!("{}/v1/tasks/{}", gw.http_addr, task_id))
        .send()
        .await
        .unwrap();

    assert_eq!(status_resp.status(), 200, "status should return 200");
    let status_body: serde_json::Value = status_resp.json().await.unwrap();
    assert_eq!(status_body["state"], "pending");
    assert_eq!(status_body["metadata"]["env"], "test");

    // Cleanup
    flush_keys(
        "redis://127.0.0.1:6379",
        &[
            format!("task:{}", task_id),
            "tasks:test-submit-http".to_string(),
        ],
    )
    .await;
}

// ============================================================================
// Test: Full lifecycle (submit -> poll -> report -> retrieve) via gRPC
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_full_lifecycle_grpc() {
    let gw = start_test_gateway("full_lifecycle").await;

    let mut task_client = TaskServiceClient::connect(gw.grpc_addr.clone())
        .await
        .unwrap();

    // Submit a task
    let submit_resp = task_client
        .submit_task(SubmitTaskRequest {
            service_name: "test-lifecycle".to_string(),
            payload: r#"{"data":"work-payload"}"#.to_string(),
            metadata: HashMap::new(),
            callback_url: String::new(),
        })
        .await
        .unwrap()
        .into_inner();

    let task_id = submit_resp.task_id.clone();

    // Spawn a "fake node" that polls, receives the task, and reports result
    let grpc_addr = gw.grpc_addr.clone();
    let task_id_clone = task_id.clone();
    let node_handle = tokio::spawn(async move {
        let mut node_client = NodeServiceClient::connect(grpc_addr.clone()).await.unwrap();

        let mut stream = node_client
            .poll_tasks(PollTasksRequest {
                service_name: "test-lifecycle".to_string(),
                node_id: "test-node-1".to_string(),
            })
            .await
            .unwrap()
            .into_inner();

        // Receive the first task assignment
        let assignment = tokio::time::timeout(Duration::from_secs(10), stream.message())
            .await
            .expect("should receive task within timeout")
            .expect("stream should not error")
            .expect("should receive a task assignment");

        assert_eq!(assignment.task_id, task_id_clone, "task_id should match");
        assert_eq!(assignment.payload, r#"{"data":"work-payload"}"#, "payload should match");

        // Report result
        let mut report_client = NodeServiceClient::connect(grpc_addr).await.unwrap();
        let ack = report_client
            .report_result(ReportResultRequest {
                task_id: task_id_clone,
                success: true,
                result: r#"{"output":"done-result"}"#.to_string(),
                error_message: String::new(),
                node_id: String::new(),
                service_name: String::new(),
            })
            .await
            .unwrap()
            .into_inner();

        assert!(ack.acknowledged, "result should be acknowledged");
    });

    // Wait for the fake node to complete
    tokio::time::timeout(Duration::from_secs(15), node_handle)
        .await
        .expect("node should complete within timeout")
        .expect("node task should not panic");

    // Brief delay for Redis state to settle
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Verify final task state
    let status = task_client
        .get_task_status(GetTaskStatusRequest {
            task_id: task_id.clone(),
        })
        .await
        .unwrap()
        .into_inner();

    // State should be COMPLETED (4)
    assert_eq!(status.state, 4, "task state should be COMPLETED (4)");
    assert_eq!(status.result, r#"{"output":"done-result"}"#, "result should match");

    // Cleanup
    flush_keys(
        "redis://127.0.0.1:6379",
        &[
            format!("task:{}", task_id),
            "tasks:test-lifecycle".to_string(),
        ],
    )
    .await;
}

// ============================================================================
// Test: Node disconnect does not crash gateway
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_node_disconnect_detection() {
    let gw = start_test_gateway("disconnect").await;

    let mut node_client = NodeServiceClient::connect(gw.grpc_addr.clone())
        .await
        .unwrap();

    let stream = node_client
        .poll_tasks(PollTasksRequest {
            service_name: "test-disconnect".to_string(),
            node_id: "disconnect-node".to_string(),
        })
        .await
        .unwrap()
        .into_inner();

    // Drop the stream and client to simulate disconnect
    drop(stream);
    drop(node_client);

    // Brief wait and verify gateway is still responsive
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Gateway should still accept new connections
    let mut task_client = TaskServiceClient::connect(gw.grpc_addr.clone())
        .await
        .unwrap();

    let resp = task_client
        .submit_task(SubmitTaskRequest {
            service_name: "test-disconnect".to_string(),
            payload: r#"{"data":"still-works"}"#.to_string(),
            metadata: HashMap::new(),
            callback_url: String::new(),
        })
        .await
        .unwrap()
        .into_inner();

    assert!(
        !resp.task_id.is_empty(),
        "gateway should still accept tasks after node disconnect"
    );

    // Cleanup
    flush_keys(
        "redis://127.0.0.1:6379",
        &[
            format!("task:{}", resp.task_id),
            "tasks:test-disconnect".to_string(),
        ],
    )
    .await;
}

// ============================================================================
// Test: Service isolation -- different services get different tasks
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_service_isolation() {
    let gw = start_test_gateway("isolation").await;

    let mut task_client = TaskServiceClient::connect(gw.grpc_addr.clone())
        .await
        .unwrap();

    // Submit task to svc-a
    let resp_a = task_client
        .submit_task(SubmitTaskRequest {
            service_name: "test-iso-svc-a".to_string(),
            payload: r#"{"data":"payload-a"}"#.to_string(),
            metadata: HashMap::new(),
            callback_url: String::new(),
        })
        .await
        .unwrap()
        .into_inner();

    // Submit task to svc-b
    let resp_b = task_client
        .submit_task(SubmitTaskRequest {
            service_name: "test-iso-svc-b".to_string(),
            payload: r#"{"data":"payload-b"}"#.to_string(),
            metadata: HashMap::new(),
            callback_url: String::new(),
        })
        .await
        .unwrap()
        .into_inner();

    // Node polls for svc-a -- should only get svc-a task
    let grpc_addr = gw.grpc_addr.clone();
    let task_id_a = resp_a.task_id.clone();
    let node_a = tokio::spawn(async move {
        let mut client = NodeServiceClient::connect(grpc_addr).await.unwrap();
        let mut stream = client
            .poll_tasks(PollTasksRequest {
                service_name: "test-iso-svc-a".to_string(),
                node_id: "node-a".to_string(),
            })
            .await
            .unwrap()
            .into_inner();

        let assignment = tokio::time::timeout(Duration::from_secs(10), stream.message())
            .await
            .expect("should get task within timeout")
            .expect("stream ok")
            .expect("got assignment");

        assert_eq!(assignment.task_id, task_id_a, "svc-a node should get svc-a task");
        assert_eq!(assignment.payload, r#"{"data":"payload-a"}"#);
    });

    // Node polls for svc-b -- should only get svc-b task
    let grpc_addr = gw.grpc_addr.clone();
    let task_id_b = resp_b.task_id.clone();
    let node_b = tokio::spawn(async move {
        let mut client = NodeServiceClient::connect(grpc_addr).await.unwrap();
        let mut stream = client
            .poll_tasks(PollTasksRequest {
                service_name: "test-iso-svc-b".to_string(),
                node_id: "node-b".to_string(),
            })
            .await
            .unwrap()
            .into_inner();

        let assignment = tokio::time::timeout(Duration::from_secs(10), stream.message())
            .await
            .expect("should get task within timeout")
            .expect("stream ok")
            .expect("got assignment");

        assert_eq!(assignment.task_id, task_id_b, "svc-b node should get svc-b task");
        assert_eq!(assignment.payload, r#"{"data":"payload-b"}"#);
    });

    tokio::time::timeout(Duration::from_secs(15), async {
        node_a.await.unwrap();
        node_b.await.unwrap();
    })
    .await
    .expect("both nodes should complete");

    // Cleanup
    flush_keys(
        "redis://127.0.0.1:6379",
        &[
            format!("task:{}", resp_a.task_id),
            format!("task:{}", resp_b.task_id),
            "tasks:test-iso-svc-a".to_string(),
            "tasks:test-iso-svc-b".to_string(),
        ],
    )
    .await;
}

// ============================================================================
// Test: Task not found returns appropriate errors
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_task_not_found() {
    let gw = start_test_gateway("not_found").await;

    // gRPC: should return NOT_FOUND
    let mut grpc_client = TaskServiceClient::connect(gw.grpc_addr.clone())
        .await
        .unwrap();

    let grpc_result = grpc_client
        .get_task_status(GetTaskStatusRequest {
            task_id: "nonexistent-task-id".to_string(),
        })
        .await;

    assert!(grpc_result.is_err(), "gRPC should return error for missing task");
    let status = grpc_result.unwrap_err();
    assert_eq!(
        status.code(),
        tonic::Code::NotFound,
        "gRPC error should be NOT_FOUND"
    );

    // HTTP: should return 404
    let http_client = reqwest::Client::new();
    let http_resp = http_client
        .get(format!("{}/v1/tasks/nonexistent-task-id", gw.http_addr))
        .send()
        .await
        .unwrap();

    assert_eq!(http_resp.status(), 404, "HTTP should return 404 for missing task");
}
