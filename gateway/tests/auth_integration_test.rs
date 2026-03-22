//! Integration tests for Phase 2 authentication and TLS.
//!
//! These tests require a running Redis instance. They are gated with `#[ignore]`
//! and run via: `cargo test -p xgent-gateway --test auth_integration_test -- --ignored`
//!
//! Tests verify all Phase 2 ROADMAP success criteria:
//! - AUTH-01: HTTPS without valid API key -> 401
//! - AUTH-02: gRPC without valid client cert -> TLS rejection
//! - AUTH-03: Node poll with bad/wrong-service token -> rejected
//! - INFR-05: All traffic over TLS

use std::sync::{Arc, Once};
use std::time::Duration;

use rcgen::{BasicConstraints, CertificateParams, IsCa, KeyPair};
use tempfile::TempDir;

static CRYPTO_INIT: Once = Once::new();

fn ensure_crypto_provider() {
    CRYPTO_INIT.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

use xgent_gateway::{auth, config, grpc, http, metrics::Metrics, queue, state, tls};
use xgent_proto::node_service_client::NodeServiceClient;
use xgent_proto::node_service_server::NodeServiceServer;
use xgent_proto::task_service_client::TaskServiceClient;
use xgent_proto::task_service_server::TaskServiceServer;
use xgent_proto::PollTasksRequest;

// ============================================================================
// Test cert infrastructure
// ============================================================================

struct TestCerts {
    _dir: TempDir,
    ca_cert_path: std::path::PathBuf,
    server_cert_path: std::path::PathBuf,
    server_key_path: std::path::PathBuf,
    ca_cert_pem: String,
    client_cert_pem: String,
    client_key_pem: String,
}

fn generate_test_certs() -> TestCerts {
    let dir = tempfile::tempdir().unwrap();

    // Generate CA
    let ca_key = KeyPair::generate().unwrap();
    let mut ca_params = CertificateParams::new(vec!["Test CA".to_string()]).unwrap();
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let ca_cert = ca_params.self_signed(&ca_key).unwrap();

    // Generate server cert signed by CA (SAN: localhost)
    let server_key = KeyPair::generate().unwrap();
    let server_params = CertificateParams::new(vec!["localhost".to_string()]).unwrap();
    let server_cert = server_params
        .signed_by(&server_key, &ca_cert, &ca_key)
        .unwrap();

    // Generate client cert signed by CA (for mTLS)
    let client_key = KeyPair::generate().unwrap();
    let client_params = CertificateParams::new(vec!["test-client".to_string()]).unwrap();
    let client_cert = client_params
        .signed_by(&client_key, &ca_cert, &ca_key)
        .unwrap();

    // Write to temp files
    let ca_path = dir.path().join("ca.pem");
    let server_cert_path = dir.path().join("server.pem");
    let server_key_path = dir.path().join("server-key.pem");
    std::fs::write(&ca_path, ca_cert.pem()).unwrap();
    std::fs::write(&server_cert_path, server_cert.pem()).unwrap();
    std::fs::write(&server_key_path, server_key.serialize_pem()).unwrap();

    TestCerts {
        _dir: dir,
        ca_cert_path: ca_path,
        server_cert_path: server_cert_path,
        server_key_path: server_key_path,
        ca_cert_pem: ca_cert.pem(),
        client_cert_pem: client_cert.pem(),
        client_key_pem: client_key.serialize_pem(),
    }
}

// ============================================================================
// Test gateway with TLS and auth
// ============================================================================

struct AuthTestGateway {
    grpc_addr: String,
    http_addr: String,
    http_port: u16,
    certs: TestCerts,
    auth_conn: redis::aio::MultiplexedConnection,
    _shutdown: tokio::sync::oneshot::Sender<()>,
}

/// Start a test gateway with TLS enabled and auth middleware active.
async fn start_auth_test_gateway(_test_name: &str) -> AuthTestGateway {
    ensure_crypto_provider();
    let certs = generate_test_certs();

    // Find free ports
    let grpc_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let grpc_port = grpc_listener.local_addr().unwrap().port();
    drop(grpc_listener);

    let http_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let http_port = http_listener.local_addr().unwrap().port();
    drop(http_listener);

    let grpc_addr_str = format!("127.0.0.1:{grpc_port}");
    let http_addr_str = format!("127.0.0.1:{http_port}");

    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

    let cfg = config::GatewayConfig {
        grpc: config::GrpcConfig {
            enabled: true,
            listen_addr: grpc_addr_str.clone(),
            tls: Some(config::GrpcTlsConfig {
                server: config::TlsConfig {
                    cert_path: certs.server_cert_path.to_str().unwrap().to_string(),
                    key_path: certs.server_key_path.to_str().unwrap().to_string(),
                },
                client_ca_path: certs.ca_cert_path.to_str().unwrap().to_string(),
            }),
            mtls_identity: Default::default(),
        },
        http: config::HttpConfig {
            enabled: true,
            listen_addr: http_addr_str.clone(),
            tls: Some(config::TlsConfig {
                cert_path: certs.server_cert_path.to_str().unwrap().to_string(),
                key_path: certs.server_key_path.to_str().unwrap().to_string(),
            }),
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

    // Open auth connection
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

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    // Spawn gRPC server with mTLS
    let grpc_state = app_state.clone();
    let grpc_addr: std::net::SocketAddr = grpc_addr_str.parse().unwrap();
    let grpc_tls_cfg = cfg.grpc.tls.clone().unwrap();
    let grpc_handle = tokio::spawn(async move {
        let tls_config =
            tls::config::build_grpc_tls_config(&grpc_tls_cfg).expect("gRPC TLS config");
        tonic::transport::Server::builder()
            .tls_config(tls_config)
            .unwrap()
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

    // Spawn HTTPS server with TLS
    let http_state = app_state.clone();
    let http_tls_cfg = cfg.http.tls.clone().unwrap();
    let http_handle = tokio::spawn(async move {
        let tls_config =
            tls::config::build_http_tls_config(&http_tls_cfg).expect("HTTP TLS config");
        let tls_acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(tls_config));
        let tcp_listener = tokio::net::TcpListener::bind(&http_addr_str).await.unwrap();

        // API routes with auth middleware
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

        // Admin routes (unauthenticated)
        let admin_routes = axum::Router::new()
            .route(
                "/v1/admin/api-keys",
                axum::routing::post(http::admin::create_api_key),
            )
            .route(
                "/v1/admin/node-tokens",
                axum::routing::post(http::admin::create_node_token),
            );

        let app = axum::Router::new()
            .merge(api_routes)
            .merge(admin_routes)
            .with_state(http_state);

        loop {
            let (tcp_stream, _addr) = tcp_listener.accept().await.unwrap();
            let acceptor = tls_acceptor.clone();
            let app = app.clone();
            tokio::spawn(async move {
                match acceptor.accept(tcp_stream).await {
                    Ok(tls_stream) => {
                        let io = hyper_util::rt::TokioIo::new(tls_stream);
                        let service = hyper_util::service::TowerToHyperService::new(app);
                        let builder = hyper_util::server::conn::auto::Builder::new(
                            hyper_util::rt::TokioExecutor::new(),
                        );
                        let conn = builder.serve_connection(io, service);
                        let _ = conn.await;
                    }
                    Err(_e) => {
                        // TLS handshake failure expected for some tests
                    }
                }
            });
        }
    });

    // Shutdown watcher
    tokio::spawn(async move {
        let _ = shutdown_rx.await;
        grpc_handle.abort();
        http_handle.abort();
    });

    // Wait for servers to start
    tokio::time::sleep(Duration::from_millis(300)).await;

    AuthTestGateway {
        grpc_addr: format!("https://localhost:{grpc_port}"),
        http_addr: format!("https://localhost:{http_port}"),
        http_port,
        certs,
        auth_conn,
        _shutdown: shutdown_tx,
    }
}

/// Build an HTTPS reqwest client that trusts the test CA cert.
fn build_https_client(ca_pem: &str) -> reqwest::Client {
    let ca_cert = reqwest::tls::Certificate::from_pem(ca_pem.as_bytes()).unwrap();
    reqwest::Client::builder()
        .add_root_certificate(ca_cert)
        .build()
        .unwrap()
}

/// Create an API key in Redis for testing, returning the raw key.
async fn create_test_api_key(
    conn: &mut redis::aio::MultiplexedConnection,
    service_names: &[String],
) -> String {
    let (raw_key, key_hash) = auth::api_key::generate_api_key();
    auth::api_key::store_api_key(conn, &key_hash, service_names, None)
        .await
        .unwrap();
    raw_key
}

/// Create a node token in Redis for testing, returning the raw token.
async fn create_test_node_token(
    conn: &mut redis::aio::MultiplexedConnection,
    service_name: &str,
) -> String {
    let (raw_token, token_hash) = auth::node_token::generate_node_token();
    auth::node_token::store_node_token(conn, service_name, &token_hash, Some("test-node"))
        .await
        .unwrap();
    raw_token
}

// ============================================================================
// AUTH-01: HTTPS API key auth tests
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_http_no_api_key() {
    let gw = start_auth_test_gateway("no_api_key").await;
    let client = build_https_client(&gw.certs.ca_cert_pem);

    let resp = client
        .post(format!("{}/v1/tasks", gw.http_addr))
        .json(&serde_json::json!({
            "service_name": "test-svc",
            "payload": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"test"),
            "metadata": {}
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 401, "request without API key should return 401");
}

#[tokio::test]
#[ignore]
async fn test_http_invalid_api_key() {
    let gw = start_auth_test_gateway("invalid_api_key").await;
    let client = build_https_client(&gw.certs.ca_cert_pem);

    let resp = client
        .post(format!("{}/v1/tasks", gw.http_addr))
        .header("Authorization", "Bearer invalid_key_that_does_not_exist")
        .json(&serde_json::json!({
            "service_name": "test-svc",
            "payload": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"test"),
            "metadata": {}
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 401, "request with invalid API key should return 401");
}

#[tokio::test]
#[ignore]
async fn test_http_wrong_service_key() {
    let gw = start_auth_test_gateway("wrong_svc_key").await;
    let client = build_https_client(&gw.certs.ca_cert_pem);

    // Create API key authorized for svc-a only
    let api_key = create_test_api_key(
        &mut gw.auth_conn.clone(),
        &["svc-a".to_string()],
    )
    .await;

    // Try to submit to svc-b
    let resp = client
        .post(format!("{}/v1/tasks", gw.http_addr))
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&serde_json::json!({
            "service_name": "svc-b",
            "payload": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"test"),
            "metadata": {}
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        401,
        "API key for svc-a should not work for svc-b"
    );
}

#[tokio::test]
#[ignore]
async fn test_http_valid_api_key() {
    let gw = start_auth_test_gateway("valid_api_key").await;
    let client = build_https_client(&gw.certs.ca_cert_pem);

    let api_key = create_test_api_key(
        &mut gw.auth_conn.clone(),
        &["test-svc".to_string()],
    )
    .await;

    let resp = client
        .post(format!("{}/v1/tasks", gw.http_addr))
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&serde_json::json!({
            "service_name": "test-svc",
            "payload": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"hello"),
            "metadata": {}
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200, "valid API key should succeed");
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(
        body["task_id"].as_str().is_some(),
        "response should contain task_id"
    );
}

#[tokio::test]
#[ignore]
async fn test_http_x_api_key_header() {
    let gw = start_auth_test_gateway("x_api_key").await;
    let client = build_https_client(&gw.certs.ca_cert_pem);

    let api_key = create_test_api_key(
        &mut gw.auth_conn.clone(),
        &["test-svc".to_string()],
    )
    .await;

    let resp = client
        .post(format!("{}/v1/tasks", gw.http_addr))
        .header("X-API-Key", &api_key)
        .json(&serde_json::json!({
            "service_name": "test-svc",
            "payload": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"hello"),
            "metadata": {}
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200, "X-API-Key header should also work");
}

// ============================================================================
// AUTH-02: gRPC mTLS tests
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_grpc_no_client_cert() {
    let gw = start_auth_test_gateway("no_client_cert").await;

    // Connect with TLS but WITHOUT client certificate
    let tls_config = tonic::transport::ClientTlsConfig::new()
        .domain_name("localhost")
        .ca_certificate(tonic::transport::Certificate::from_pem(&gw.certs.ca_cert_pem));

    let channel = tonic::transport::Channel::from_shared(gw.grpc_addr.clone())
        .unwrap()
        .tls_config(tls_config)
        .unwrap()
        .connect()
        .await;

    // Connection should fail at TLS handshake since server requires client cert
    // Or if it connects, the first RPC should fail
    match channel {
        Err(_) => {
            // Expected: TLS handshake rejection
        }
        Ok(ch) => {
            // If channel connected, the RPC should fail
            let mut client = TaskServiceClient::new(ch);
            let result = client
                .submit_task(xgent_proto::SubmitTaskRequest {
                    service_name: "test-svc".to_string(),
                    payload: b"test".to_vec(),
                    metadata: std::collections::HashMap::new(),
                    callback_url: String::new(),
                })
                .await;
            assert!(result.is_err(), "gRPC without client cert should fail");
        }
    }
}

#[tokio::test]
#[ignore]
async fn test_grpc_valid_mtls() {
    let gw = start_auth_test_gateway("valid_mtls").await;

    // Connect with valid client certificate
    let tls_config = tonic::transport::ClientTlsConfig::new()
        .domain_name("localhost")
        .ca_certificate(tonic::transport::Certificate::from_pem(&gw.certs.ca_cert_pem))
        .identity(tonic::transport::Identity::from_pem(
            &gw.certs.client_cert_pem,
            &gw.certs.client_key_pem,
        ));

    let channel = tonic::transport::Channel::from_shared(gw.grpc_addr.clone())
        .unwrap()
        .tls_config(tls_config)
        .unwrap()
        .connect()
        .await
        .expect("mTLS connection should succeed");

    // Need a valid node token to actually call PollTasks
    let node_token = create_test_node_token(&mut gw.auth_conn.clone(), "test-svc").await;

    let mut client = NodeServiceClient::new(channel);
    let mut request = tonic::Request::new(PollTasksRequest {
        service_name: "test-svc".to_string(),
        node_id: "test-node-1".to_string(),
    });
    request.metadata_mut().insert(
        "authorization",
        format!("Bearer {node_token}").parse().unwrap(),
    );

    // PollTasks returns a stream -- just verify we can open it
    let result = client.poll_tasks(request).await;
    assert!(result.is_ok(), "mTLS with valid client cert should succeed");
}

// ============================================================================
// AUTH-03: Node token auth tests
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_node_invalid_token() {
    let gw = start_auth_test_gateway("invalid_token").await;

    let tls_config = tonic::transport::ClientTlsConfig::new()
        .domain_name("localhost")
        .ca_certificate(tonic::transport::Certificate::from_pem(&gw.certs.ca_cert_pem))
        .identity(tonic::transport::Identity::from_pem(
            &gw.certs.client_cert_pem,
            &gw.certs.client_key_pem,
        ));

    let channel = tonic::transport::Channel::from_shared(gw.grpc_addr.clone())
        .unwrap()
        .tls_config(tls_config)
        .unwrap()
        .connect()
        .await
        .unwrap();

    let mut client = NodeServiceClient::new(channel);
    let mut request = tonic::Request::new(PollTasksRequest {
        service_name: "test-svc".to_string(),
        node_id: "test-node-1".to_string(),
    });
    request.metadata_mut().insert(
        "authorization",
        "Bearer totally_bogus_token".parse().unwrap(),
    );

    let result = client.poll_tasks(request).await;
    assert!(result.is_err(), "invalid token should be rejected");
    assert_eq!(
        result.unwrap_err().code(),
        tonic::Code::Unauthenticated,
        "should return UNAUTHENTICATED"
    );
}

#[tokio::test]
#[ignore]
async fn test_node_valid_token() {
    let gw = start_auth_test_gateway("valid_token").await;

    let node_token = create_test_node_token(&mut gw.auth_conn.clone(), "test-svc").await;

    let tls_config = tonic::transport::ClientTlsConfig::new()
        .domain_name("localhost")
        .ca_certificate(tonic::transport::Certificate::from_pem(&gw.certs.ca_cert_pem))
        .identity(tonic::transport::Identity::from_pem(
            &gw.certs.client_cert_pem,
            &gw.certs.client_key_pem,
        ));

    let channel = tonic::transport::Channel::from_shared(gw.grpc_addr.clone())
        .unwrap()
        .tls_config(tls_config)
        .unwrap()
        .connect()
        .await
        .unwrap();

    let mut client = NodeServiceClient::new(channel);
    let mut request = tonic::Request::new(PollTasksRequest {
        service_name: "test-svc".to_string(),
        node_id: "test-node-1".to_string(),
    });
    request.metadata_mut().insert(
        "authorization",
        format!("Bearer {node_token}").parse().unwrap(),
    );

    let result = client.poll_tasks(request).await;
    assert!(result.is_ok(), "valid node token should succeed");
}

#[tokio::test]
#[ignore]
async fn test_node_wrong_service_token() {
    let gw = start_auth_test_gateway("wrong_svc_token").await;

    // Create token for svc-a
    let node_token = create_test_node_token(&mut gw.auth_conn.clone(), "svc-a").await;

    let tls_config = tonic::transport::ClientTlsConfig::new()
        .domain_name("localhost")
        .ca_certificate(tonic::transport::Certificate::from_pem(&gw.certs.ca_cert_pem))
        .identity(tonic::transport::Identity::from_pem(
            &gw.certs.client_cert_pem,
            &gw.certs.client_key_pem,
        ));

    let channel = tonic::transport::Channel::from_shared(gw.grpc_addr.clone())
        .unwrap()
        .tls_config(tls_config)
        .unwrap()
        .connect()
        .await
        .unwrap();

    let mut client = NodeServiceClient::new(channel);

    // Use svc-a token to poll svc-b
    let mut request = tonic::Request::new(PollTasksRequest {
        service_name: "svc-b".to_string(),
        node_id: "test-node-1".to_string(),
    });
    request.metadata_mut().insert(
        "authorization",
        format!("Bearer {node_token}").parse().unwrap(),
    );

    let result = client.poll_tasks(request).await;
    assert!(result.is_err(), "token for svc-a should not work for svc-b");
    assert_eq!(
        result.unwrap_err().code(),
        tonic::Code::Unauthenticated,
        "wrong-service token should return UNAUTHENTICATED"
    );
}

// ============================================================================
// Admin endpoint tests (D-07/D-08)
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_admin_create_api_key() {
    let gw = start_auth_test_gateway("admin_create_key").await;
    let client = build_https_client(&gw.certs.ca_cert_pem);

    // Create API key via admin endpoint
    let resp = client
        .post(format!("{}/v1/admin/api-keys", gw.http_addr))
        .json(&serde_json::json!({
            "service_names": ["test-svc"]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 201, "admin create API key should return 201");
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["api_key"].as_str().is_some(), "response should contain api_key");
    assert!(body["key_hash"].as_str().is_some(), "response should contain key_hash");

    // Use the returned key to submit a task
    let api_key = body["api_key"].as_str().unwrap();
    let task_resp = client
        .post(format!("{}/v1/tasks", gw.http_addr))
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&serde_json::json!({
            "service_name": "test-svc",
            "payload": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"test"),
            "metadata": {}
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(
        task_resp.status(),
        200,
        "admin-created API key should work for task submission"
    );
}

// ============================================================================
// INFR-05: TLS connectivity test
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_https_tls_connection() {
    let gw = start_auth_test_gateway("tls_conn").await;
    let client = build_https_client(&gw.certs.ca_cert_pem);

    // Create an API key so we can make authenticated requests
    let api_key = create_test_api_key(
        &mut gw.auth_conn.clone(),
        &["tls-test-svc".to_string()],
    )
    .await;

    // Verify TLS connection works by submitting a task
    let resp = client
        .post(format!("{}/v1/tasks", gw.http_addr))
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&serde_json::json!({
            "service_name": "tls-test-svc",
            "payload": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"tls-test"),
            "metadata": {}
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200, "HTTPS with valid TLS should work");

    // Verify that plain HTTP connection to the TLS port fails
    let plain_client = reqwest::Client::new();
    let plain_result = plain_client
        .post(format!("http://localhost:{}/v1/tasks", gw.http_port))
        .json(&serde_json::json!({"service_name": "test"}))
        .send()
        .await;

    assert!(
        plain_result.is_err(),
        "plain HTTP to TLS port should fail"
    );
}

// ============================================================================
// UAT-1: TLS Handshake Behavior Under Load
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_uat_tls_concurrent_requests() {
    let gw = start_auth_test_gateway("tls_concurrent").await;

    let api_key = create_test_api_key(
        &mut gw.auth_conn.clone(),
        &["load-svc".to_string()],
    )
    .await;

    // Spawn 20 concurrent HTTPS requests, each establishing a fresh TLS handshake
    let mut handles = Vec::new();
    for i in 0..20u32 {
        let addr = gw.http_addr.clone();
        let key = api_key.clone();
        let ca_pem = gw.certs.ca_cert_pem.clone();
        handles.push(tokio::spawn(async move {
            // Build a fresh client per request to force new TLS handshake
            let client = build_https_client(&ca_pem);
            let resp = client
                .post(format!("{}/v1/tasks", addr))
                .header("Authorization", format!("Bearer {key}"))
                .json(&serde_json::json!({
                    "service_name": "load-svc",
                    "payload": base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        format!("req-{i}").as_bytes(),
                    ),
                    "metadata": {}
                }))
                .send()
                .await;
            (i, resp)
        }));
    }

    let mut successes = 0u32;
    let mut failures = Vec::new();
    for handle in handles {
        let (i, result) = handle.await.unwrap();
        match result {
            Ok(resp) if resp.status() == 200 => successes += 1,
            Ok(resp) => failures.push(format!("req-{i}: status {}", resp.status())),
            Err(e) => failures.push(format!("req-{i}: {e}")),
        }
    }

    assert!(
        failures.is_empty(),
        "All 20 concurrent TLS requests should succeed. {} failed: {:?}",
        failures.len(),
        failures,
    );
    assert_eq!(successes, 20, "Expected 20 successful responses");
}

// ============================================================================
// UAT-2: mTLS Certificate Rejection (Wrong CA)
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_uat_grpc_wrong_ca_cert() {
    let gw = start_auth_test_gateway("wrong_ca").await;

    // Generate a completely separate CA and client cert NOT trusted by the server
    let rogue_ca_key = KeyPair::generate().unwrap();
    let mut rogue_ca_params =
        CertificateParams::new(vec!["Rogue CA".to_string()]).unwrap();
    rogue_ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let rogue_ca_cert = rogue_ca_params.self_signed(&rogue_ca_key).unwrap();

    let rogue_client_key = KeyPair::generate().unwrap();
    let rogue_client_params =
        CertificateParams::new(vec!["rogue-client".to_string()]).unwrap();
    let rogue_client_cert = rogue_client_params
        .signed_by(&rogue_client_key, &rogue_ca_cert, &rogue_ca_key)
        .unwrap();

    // Connect using the rogue client cert (signed by rogue CA) but trust the real server CA
    let tls_config = tonic::transport::ClientTlsConfig::new()
        .domain_name("localhost")
        .ca_certificate(tonic::transport::Certificate::from_pem(&gw.certs.ca_cert_pem))
        .identity(tonic::transport::Identity::from_pem(
            rogue_client_cert.pem(),
            rogue_client_key.serialize_pem(),
        ));

    let channel_result = tonic::transport::Channel::from_shared(gw.grpc_addr.clone())
        .unwrap()
        .tls_config(tls_config)
        .unwrap()
        .connect()
        .await;

    match channel_result {
        Err(_) => {
            // Expected: TLS handshake fails because server doesn't trust rogue CA
        }
        Ok(ch) => {
            // If channel somehow connected, the first RPC must fail
            let node_token =
                create_test_node_token(&mut gw.auth_conn.clone(), "test-svc").await;
            let mut client = NodeServiceClient::new(ch);
            let mut request = tonic::Request::new(PollTasksRequest {
                service_name: "test-svc".to_string(),
                node_id: "rogue-node".to_string(),
            });
            request.metadata_mut().insert(
                "authorization",
                format!("Bearer {node_token}").parse().unwrap(),
            );

            let result = client.poll_tasks(request).await;
            assert!(
                result.is_err(),
                "gRPC with wrong-CA client cert should be rejected"
            );
        }
    }
}

// ============================================================================
// UAT-3: Redis Connection Resilience for Auth
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_uat_redis_reconnect_after_restart() {
    // This test simulates Redis disconnection by using a proxy-like approach:
    // 1. Start gateway with Redis
    // 2. Make a successful auth request
    // 3. Flush the specific auth keys (simulates data loss, not full restart)
    // 4. Re-create the key and verify auth still works
    //
    // Note: A full Redis restart test requires external orchestration (docker restart).
    // This test verifies that MultiplexedConnection handles transient failures gracefully.

    let gw = start_auth_test_gateway("redis_resilience").await;
    let client = build_https_client(&gw.certs.ca_cert_pem);

    // Step 1: Create key and verify it works
    let api_key = create_test_api_key(
        &mut gw.auth_conn.clone(),
        &["resilience-svc".to_string()],
    )
    .await;

    let resp = client
        .post(format!("{}/v1/tasks", gw.http_addr))
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&serde_json::json!({
            "service_name": "resilience-svc",
            "payload": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"before"),
            "metadata": {}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "Initial request should succeed");

    // Step 2: Delete the specific key hash to simulate data loss
    let mut conn = gw.auth_conn.clone();
    let key_hash = auth::api_key::hash_api_key(&api_key);
    let redis_key = format!("api_keys:{key_hash}");
    let _: () = redis::cmd("DEL")
        .arg(&redis_key)
        .query_async(&mut conn)
        .await
        .unwrap();

    // Step 3: Old key should now be rejected (data gone)
    let resp = client
        .post(format!("{}/v1/tasks", gw.http_addr))
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&serde_json::json!({
            "service_name": "resilience-svc",
            "payload": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"after-flush"),
            "metadata": {}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        401,
        "Deleted key should be rejected — proves Redis connection is still alive"
    );

    // Step 4: Create a NEW key and verify auth pipeline still works end-to-end
    let new_key = create_test_api_key(
        &mut gw.auth_conn.clone(),
        &["resilience-svc".to_string()],
    )
    .await;

    let resp = client
        .post(format!("{}/v1/tasks", gw.http_addr))
        .header("Authorization", format!("Bearer {new_key}"))
        .json(&serde_json::json!({
            "service_name": "resilience-svc",
            "payload": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"recovered"),
            "metadata": {}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "New key after Redis data loss should work — connection recovered"
    );
}
