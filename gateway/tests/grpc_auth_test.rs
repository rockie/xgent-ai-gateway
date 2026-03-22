//! Integration tests for Phase 6 gRPC auth hardening.
//!
//! Require a running Redis instance. Gated with #[ignore].
//! Run via: cargo test -p xgent-gateway --test grpc_auth_test -- --ignored

use std::collections::HashMap;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tonic::Code;

use xgent_gateway::{auth, config, grpc, metrics::Metrics, queue, registry, state};
use xgent_proto::node_service_client::NodeServiceClient;
use xgent_proto::node_service_server::NodeServiceServer;
use xgent_proto::task_service_client::TaskServiceClient;
use xgent_proto::task_service_server::TaskServiceServer;
use xgent_proto::{
    DrainNodeRequest, GetTaskStatusRequest, HeartbeatRequest, ReportResultRequest,
    SubmitTaskRequest,
};

// ============================================================================
// Test infrastructure
// ============================================================================

/// Atomic counter to give each test a unique Redis DB index (1..15).
static DB_COUNTER: AtomicU16 = AtomicU16::new(1);

struct TestGrpcServer {
    addr: std::net::SocketAddr,
    raw_api_key: String,
    raw_node_token: String,
    service_name: String,
    auth_conn: redis::aio::MultiplexedConnection,
}

/// Start a gRPC server on a random port with auth layers enabled.
/// Each call uses a unique Redis DB to avoid test interference.
async fn start_test_grpc_server() -> TestGrpcServer {
    let db_index = DB_COUNTER.fetch_add(1, Ordering::SeqCst);
    let base_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    // Use a unique Redis DB per test invocation
    let redis_url = format!("{base_url}/{db_index}");

    let redis_client = redis::Client::open(redis_url.as_str()).unwrap();
    let mut conn = redis_client
        .get_multiplexed_async_connection()
        .await
        .unwrap();

    // Flush this specific database for a clean test state
    redis::cmd("FLUSHDB")
        .query_async::<()>(&mut conn)
        .await
        .unwrap();

    let cfg = config::GatewayConfig {
        grpc: config::GrpcConfig {
            enabled: true,
            listen_addr: "127.0.0.1:0".to_string(),
            tls: None,
        },
        http: config::HttpConfig {
            enabled: false,
            listen_addr: "127.0.0.1:0".to_string(),
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
        admin: config::AdminConfig { token: None },
        service_defaults: config::ServiceDefaultsConfig::default(),
        callback: config::CallbackConfig::default(),
        logging: config::LoggingConfig::default(),
    };

    let redis_queue = queue::RedisQueue::new(&cfg)
        .await
        .expect("Redis must be running for integration tests");

    let auth_client = redis::Client::open(redis_url.as_str()).unwrap();
    let auth_conn = auth_client
        .get_multiplexed_async_connection()
        .await
        .unwrap();

    let app_state = Arc::new(state::AppState::new(
        redis_queue,
        cfg.clone(),
        auth_conn.clone(),
        reqwest::Client::new(),
        Metrics::new(),
    ));

    // Register test service
    let service_name = "test-svc".to_string();
    let svc_config = registry::node_health::ServiceConfig {
        name: service_name.clone(),
        description: "test service".to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        task_timeout_secs: 300,
        max_retries: 3,
        max_nodes: None,
        node_stale_after_secs: 60,
        drain_timeout_secs: 300,
    };
    let mut queue_conn = redis::Client::open(redis_url.as_str())
        .unwrap()
        .get_multiplexed_async_connection()
        .await
        .unwrap();
    registry::service::register_service(&mut conn.clone(), &svc_config, &mut queue_conn)
        .await
        .unwrap();

    // Create API key authorized for test-svc
    let (raw_api_key, key_hash) = auth::api_key::generate_api_key();
    auth::api_key::store_api_key(&mut conn.clone(), &key_hash, &[service_name.clone()], None)
        .await
        .unwrap();

    // Create node token for test-svc
    let (raw_node_token, token_hash) = auth::node_token::generate_node_token();
    auth::node_token::store_node_token(
        &mut conn.clone(),
        &service_name,
        &token_hash,
        Some("test-node"),
    )
    .await
    .unwrap();

    // Bind listener on random port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Build gRPC server with auth layers (same pattern as main.rs)
    let task_svc = TaskServiceServer::new(grpc::GrpcTaskService::new(app_state.clone()));
    let node_svc = NodeServiceServer::new(grpc::GrpcNodeService::new(app_state.clone()));

    let grpc_server = tonic::transport::Server::builder()
        .add_service(grpc::ApiKeyAuthLayer::new(task_svc, app_state.clone()))
        .add_service(grpc::NodeTokenAuthLayer::new(node_svc, app_state));

    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
    tokio::spawn(async move {
        grpc_server
            .serve_with_incoming(incoming)
            .await
            .expect("gRPC test server failed");
    });

    // Wait for server to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    TestGrpcServer {
        addr,
        raw_api_key,
        raw_node_token,
        service_name,
        auth_conn: conn,
    }
}

/// Create a TaskServiceClient connected to the test server.
async fn task_client(addr: std::net::SocketAddr) -> TaskServiceClient<tonic::transport::Channel> {
    TaskServiceClient::connect(format!("http://{addr}"))
        .await
        .expect("failed to connect task client")
}

/// Create a NodeServiceClient connected to the test server.
async fn node_client(addr: std::net::SocketAddr) -> NodeServiceClient<tonic::transport::Channel> {
    NodeServiceClient::connect(format!("http://{addr}"))
        .await
        .expect("failed to connect node client")
}

/// Build a SubmitTaskRequest for the given service name.
fn make_submit_request(service_name: &str) -> SubmitTaskRequest {
    SubmitTaskRequest {
        service_name: service_name.to_string(),
        payload: b"test-payload".to_vec(),
        metadata: HashMap::new(),
    }
}

// ============================================================================
// AUTH-01: SubmitTask API key tests
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_grpc_submit_no_api_key() {
    let gw = start_test_grpc_server().await;
    let mut client = task_client(gw.addr).await;

    let request = tonic::Request::new(make_submit_request(&gw.service_name));
    let result = client.submit_task(request).await;

    assert!(result.is_err(), "submit without API key should fail");
    let status = result.unwrap_err();
    assert_eq!(status.code(), Code::Unauthenticated);
    assert_eq!(status.message(), "unauthorized");
}

#[tokio::test]
#[ignore]
async fn test_grpc_submit_invalid_api_key() {
    let gw = start_test_grpc_server().await;
    let mut client = task_client(gw.addr).await;

    let mut request = tonic::Request::new(make_submit_request(&gw.service_name));
    request.metadata_mut().insert(
        "authorization",
        "Bearer invalid_key_12345".parse().unwrap(),
    );
    let result = client.submit_task(request).await;

    assert!(result.is_err(), "submit with invalid API key should fail");
    let status = result.unwrap_err();
    assert_eq!(status.code(), Code::Unauthenticated);
}

#[tokio::test]
#[ignore]
async fn test_grpc_submit_valid_api_key() {
    let gw = start_test_grpc_server().await;
    let mut client = task_client(gw.addr).await;

    let mut request = tonic::Request::new(make_submit_request(&gw.service_name));
    request.metadata_mut().insert(
        "authorization",
        format!("Bearer {}", gw.raw_api_key).parse().unwrap(),
    );
    let result = client.submit_task(request).await;

    assert!(
        result.is_ok(),
        "submit with valid API key should succeed: {:?}",
        result.err()
    );
    let resp = result.unwrap().into_inner();
    assert!(!resp.task_id.is_empty(), "task_id should be non-empty");
}

#[tokio::test]
#[ignore]
async fn test_grpc_submit_wrong_service() {
    let gw = start_test_grpc_server().await;
    let mut client = task_client(gw.addr).await;

    // Create API key authorized ONLY for "other-svc"
    // (other-svc does NOT need to be registered -- the auth layer passes
    //  because the key is valid, and the handler rejects because key's
    //  service_names does not include "test-svc")
    let (other_key, other_hash) = auth::api_key::generate_api_key();
    auth::api_key::store_api_key(
        &mut gw.auth_conn.clone(),
        &other_hash,
        &["other-svc".to_string()],
        None,
    )
    .await
    .unwrap();

    // Submit to "test-svc" with key that only has "other-svc"
    let mut request = tonic::Request::new(make_submit_request("test-svc"));
    request.metadata_mut().insert(
        "authorization",
        format!("Bearer {other_key}").parse().unwrap(),
    );
    let result = client.submit_task(request).await;

    assert!(result.is_err(), "submit to unauthorized service should fail");
    let status = result.unwrap_err();
    assert_eq!(
        status.code(),
        Code::PermissionDenied,
        "wrong-service should return PermissionDenied, not Unauthenticated"
    );
}

// ============================================================================
// RSLT-01: GetTaskStatus API key tests
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_grpc_status_no_key() {
    let gw = start_test_grpc_server().await;
    let mut client = task_client(gw.addr).await;

    let request = tonic::Request::new(GetTaskStatusRequest {
        task_id: "nonexistent-task-id".to_string(),
    });
    let result = client.get_task_status(request).await;

    assert!(
        result.is_err(),
        "get_task_status without API key should fail"
    );
    let status = result.unwrap_err();
    assert_eq!(status.code(), Code::Unauthenticated);
}

#[tokio::test]
#[ignore]
async fn test_grpc_status_wrong_service() {
    let gw = start_test_grpc_server().await;
    let mut client = task_client(gw.addr).await;

    // Submit a task to test-svc with the valid key
    let mut submit_req = tonic::Request::new(make_submit_request(&gw.service_name));
    submit_req.metadata_mut().insert(
        "authorization",
        format!("Bearer {}", gw.raw_api_key).parse().unwrap(),
    );
    let submit_resp = client.submit_task(submit_req).await.unwrap().into_inner();
    let task_id = submit_resp.task_id;

    // Create API key authorized ONLY for "other-svc"
    let (other_key, other_hash) = auth::api_key::generate_api_key();
    auth::api_key::store_api_key(
        &mut gw.auth_conn.clone(),
        &other_hash,
        &["other-svc".to_string()],
        None,
    )
    .await
    .unwrap();

    // Try to get status of test-svc task with other-svc key
    let mut status_req = tonic::Request::new(GetTaskStatusRequest { task_id });
    status_req.metadata_mut().insert(
        "authorization",
        format!("Bearer {other_key}").parse().unwrap(),
    );
    let result = client.get_task_status(status_req).await;

    assert!(
        result.is_err(),
        "get_task_status with wrong-service key should fail"
    );
    let status = result.unwrap_err();
    assert_eq!(
        status.code(),
        Code::PermissionDenied,
        "wrong-service status check should return PermissionDenied"
    );
}

#[tokio::test]
#[ignore]
async fn test_grpc_status_valid() {
    let gw = start_test_grpc_server().await;
    let mut client = task_client(gw.addr).await;

    // Submit a task first
    let mut submit_req = tonic::Request::new(make_submit_request(&gw.service_name));
    submit_req.metadata_mut().insert(
        "authorization",
        format!("Bearer {}", gw.raw_api_key).parse().unwrap(),
    );
    let submit_resp = client.submit_task(submit_req).await.unwrap().into_inner();
    let task_id = submit_resp.task_id.clone();

    // Get status with same valid key
    let mut status_req = tonic::Request::new(GetTaskStatusRequest {
        task_id: task_id.clone(),
    });
    status_req.metadata_mut().insert(
        "authorization",
        format!("Bearer {}", gw.raw_api_key).parse().unwrap(),
    );
    let result = client.get_task_status(status_req).await;

    assert!(
        result.is_ok(),
        "get_task_status with valid key should succeed: {:?}",
        result.err()
    );
    let resp = result.unwrap().into_inner();
    assert_eq!(resp.task_id, task_id);
}

// ============================================================================
// AUTH-03/NODE-04: ReportResult node token tests
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_grpc_report_no_token() {
    let gw = start_test_grpc_server().await;
    let mut client = node_client(gw.addr).await;

    let request = tonic::Request::new(ReportResultRequest {
        task_id: "some-task-id".to_string(),
        success: true,
        result: b"result".to_vec(),
        error_message: String::new(),
    });
    let result = client.report_result(request).await;

    assert!(result.is_err(), "report_result without token should fail");
    let status = result.unwrap_err();
    assert_eq!(status.code(), Code::Unauthenticated);
}

#[tokio::test]
#[ignore]
async fn test_grpc_report_valid() {
    let gw = start_test_grpc_server().await;

    // Submit a task first (via task client with API key)
    let mut task_cl = task_client(gw.addr).await;
    let mut submit_req = tonic::Request::new(make_submit_request(&gw.service_name));
    submit_req.metadata_mut().insert(
        "authorization",
        format!("Bearer {}", gw.raw_api_key).parse().unwrap(),
    );
    let submit_resp = task_cl.submit_task(submit_req).await.unwrap().into_inner();
    let task_id = submit_resp.task_id;

    // Report result with valid node token
    let mut node_cl = node_client(gw.addr).await;
    let mut report_req = tonic::Request::new(ReportResultRequest {
        task_id,
        success: true,
        result: b"test-result".to_vec(),
        error_message: String::new(),
    });
    report_req.metadata_mut().insert(
        "authorization",
        format!("Bearer {}", gw.raw_node_token).parse().unwrap(),
    );
    report_req
        .metadata_mut()
        .insert("x-service-name", gw.service_name.parse().unwrap());
    let result = node_cl.report_result(report_req).await;

    // Auth passed if we get Ok or a business-logic error (FailedPrecondition).
    // The task is in "pending" state (not yet assigned to a node), so report_result
    // may reject the state transition -- but that proves auth was accepted.
    match &result {
        Ok(resp) => {
            assert!(resp.get_ref().acknowledged);
        }
        Err(status) => {
            assert_ne!(
                status.code(),
                Code::Unauthenticated,
                "valid token should not be rejected as unauthenticated"
            );
            // FailedPrecondition is expected: task is pending, not assigned
            assert_eq!(
                status.code(),
                Code::FailedPrecondition,
                "expected FailedPrecondition for pending task, got {:?}",
                status.code()
            );
        }
    }
}

// ============================================================================
// AUTH-03/NODE-03: Heartbeat node token tests
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_grpc_heartbeat_no_token() {
    let gw = start_test_grpc_server().await;
    let mut client = node_client(gw.addr).await;

    let request = tonic::Request::new(HeartbeatRequest {
        service_name: "test-svc".to_string(),
        node_id: "test-node-1".to_string(),
    });
    let result = client.heartbeat(request).await;

    assert!(result.is_err(), "heartbeat without token should fail");
    let status = result.unwrap_err();
    assert_eq!(status.code(), Code::Unauthenticated);
}

#[tokio::test]
#[ignore]
async fn test_grpc_heartbeat_valid() {
    let gw = start_test_grpc_server().await;
    let mut client = node_client(gw.addr).await;

    let mut request = tonic::Request::new(HeartbeatRequest {
        service_name: gw.service_name.clone(),
        node_id: "test-node-1".to_string(),
    });
    request.metadata_mut().insert(
        "authorization",
        format!("Bearer {}", gw.raw_node_token).parse().unwrap(),
    );
    request
        .metadata_mut()
        .insert("x-service-name", gw.service_name.parse().unwrap());
    let result = client.heartbeat(request).await;

    assert!(
        result.is_ok(),
        "heartbeat with valid token should succeed: {:?}",
        result.err()
    );
    let resp = result.unwrap().into_inner();
    assert!(resp.acknowledged);
}

// ============================================================================
// AUTH-03/NODE-06: DrainNode node token tests
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_grpc_drain_no_token() {
    let gw = start_test_grpc_server().await;
    let mut client = node_client(gw.addr).await;

    let request = tonic::Request::new(DrainNodeRequest {
        service_name: "test-svc".to_string(),
        node_id: "test-node-1".to_string(),
    });
    let result = client.drain_node(request).await;

    assert!(result.is_err(), "drain_node without token should fail");
    let status = result.unwrap_err();
    assert_eq!(status.code(), Code::Unauthenticated);
}

#[tokio::test]
#[ignore]
async fn test_grpc_drain_valid() {
    let gw = start_test_grpc_server().await;
    let mut client = node_client(gw.addr).await;

    // Register the node first so drain can find it
    registry::node_health::register_or_update_node(
        &mut gw.auth_conn.clone(),
        &gw.service_name,
        "test-node-1",
    )
    .await
    .unwrap();

    let mut request = tonic::Request::new(DrainNodeRequest {
        service_name: gw.service_name.clone(),
        node_id: "test-node-1".to_string(),
    });
    request.metadata_mut().insert(
        "authorization",
        format!("Bearer {}", gw.raw_node_token).parse().unwrap(),
    );
    request
        .metadata_mut()
        .insert("x-service-name", gw.service_name.parse().unwrap());
    let result = client.drain_node(request).await;

    assert!(
        result.is_ok(),
        "drain_node with valid token should succeed: {:?}",
        result.err()
    );
    let resp = result.unwrap().into_inner();
    assert!(resp.acknowledged);
}
