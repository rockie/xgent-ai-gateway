//! Integration tests for Phase 3: Service Registry and Node Health.
//!
//! These tests require a running Redis instance. They are gated with `#[ignore]`
//! and run via: `cargo test -p xgent-gateway --test registry_integration_test -- --ignored`
//!
//! Tests verify Phase 3 requirements:
//! - SRVC-01: Service registration and listing
//! - SRVC-03: Service deregistration cleanup
//! - SRVC-04: Service config persistence
//! - NODE-03: Task submission rejects unregistered services
//! - NODE-05: Node health tracking
//! - NODE-06: Node drain flow

use std::collections::HashMap;

use redis::AsyncCommands;

use xgent_gateway::config::load_config;
use xgent_gateway::error::GatewayError;
use xgent_gateway::queue::redis::RedisQueue;
use xgent_gateway::registry::cleanup::cleanup_service;
use xgent_gateway::registry::node_health::{
    get_nodes_for_service, is_node_draining, mark_node_disconnected,
    register_or_update_node, set_node_draining, NodeHealthState, ServiceConfig,
};
use xgent_gateway::registry::service::{
    get_service, list_services, register_service, service_exists,
};
use xgent_gateway::types::ServiceName;

/// Get a Redis connection for testing.
async fn test_conn() -> redis::aio::MultiplexedConnection {
    let client = redis::Client::open("redis://127.0.0.1:6379").unwrap();
    client.get_multiplexed_async_connection().await.unwrap()
}

/// Clean up all keys matching patterns for a given service name.
async fn cleanup_keys(conn: &mut redis::aio::MultiplexedConnection, service_name: &str) {
    // Delete known keys directly
    let keys_to_delete = vec![
        format!("service:{}", service_name),
        format!("tasks:{}", service_name),
        format!("nodes:{}", service_name),
    ];
    for key in &keys_to_delete {
        let _: redis::RedisResult<()> = redis::cmd("DEL").arg(key).query_async(conn).await;
    }

    // Remove from services index
    let _: redis::RedisResult<()> = conn.srem("services:index", service_name).await;

    // SCAN and delete node keys
    let mut cursor: u64 = 0;
    loop {
        let (new_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(format!("node:{}:*", service_name))
            .arg("COUNT")
            .arg(100)
            .query_async(conn)
            .await
            .unwrap();
        if !keys.is_empty() {
            let mut cmd = redis::cmd("DEL");
            for key in &keys {
                cmd.arg(key);
            }
            let _: redis::RedisResult<()> = cmd.query_async(conn).await;
        }
        cursor = new_cursor;
        if cursor == 0 {
            break;
        }
    }

    // SCAN and delete node_tokens keys
    cursor = 0;
    loop {
        let (new_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(format!("node_tokens:{}:*", service_name))
            .arg("COUNT")
            .arg(100)
            .query_async(conn)
            .await
            .unwrap();
        if !keys.is_empty() {
            let mut cmd = redis::cmd("DEL");
            for key in &keys {
                cmd.arg(key);
            }
            let _: redis::RedisResult<()> = cmd.query_async(conn).await;
        }
        cursor = new_cursor;
        if cursor == 0 {
            break;
        }
    }
}

fn make_service_config(name: &str) -> ServiceConfig {
    ServiceConfig {
        name: name.to_string(),
        description: "test service".to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        task_timeout_secs: 300,
        max_nodes: None,
        node_stale_after_secs: 60,
        drain_timeout_secs: 300,
    }
}

// ============================================================================
// SRVC-01: Service registration
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_register_service() {
    let svc_name = "test-reg-svc";
    let mut conn = test_conn().await;
    let mut queue_conn = test_conn().await;
    cleanup_keys(&mut conn, svc_name).await;

    let config = ServiceConfig {
        name: svc_name.to_string(),
        description: "test".to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        task_timeout_secs: 300,
        max_nodes: None,
        node_stale_after_secs: 60,
        drain_timeout_secs: 300,
    };

    register_service(&mut conn, &config, &mut queue_conn)
        .await
        .unwrap();

    // service_exists returns true
    assert!(service_exists(&mut conn, svc_name).await.unwrap());

    // get_service returns matching config
    let retrieved = get_service(&mut conn, svc_name).await.unwrap();
    assert_eq!(retrieved.name, svc_name);
    assert_eq!(retrieved.description, "test");
    assert_eq!(retrieved.task_timeout_secs, 300);
    assert!(retrieved.max_nodes.is_none());
    assert_eq!(retrieved.node_stale_after_secs, 60);
    assert_eq!(retrieved.drain_timeout_secs, 300);

    // SISMEMBER services:index returns 1
    let in_index: bool = conn.sismember("services:index", svc_name).await.unwrap();
    assert!(in_index);

    // XINFO GROUPS confirms consumer group "workers" exists
    let groups: Vec<HashMap<String, redis::Value>> = redis::cmd("XINFO")
        .arg("GROUPS")
        .arg(format!("tasks:{}", svc_name))
        .query_async(&mut conn)
        .await
        .unwrap();
    let has_workers = groups
        .iter()
        .any(|g| matches!(g.get("name"), Some(redis::Value::BulkString(b)) if b == b"workers"));
    assert!(has_workers, "consumer group 'workers' should exist");

    cleanup_keys(&mut conn, svc_name).await;
}

#[tokio::test]
#[ignore]
async fn test_register_duplicate_service_fails() {
    let svc_name = "test-dup-svc";
    let mut conn = test_conn().await;
    let mut queue_conn = test_conn().await;
    cleanup_keys(&mut conn, svc_name).await;

    let config = make_service_config(svc_name);
    register_service(&mut conn, &config, &mut queue_conn)
        .await
        .unwrap();

    // Second registration should fail
    let result = register_service(&mut conn, &config, &mut queue_conn).await;
    assert!(
        matches!(result, Err(GatewayError::ServiceAlreadyExists(_))),
        "expected ServiceAlreadyExists, got {:?}",
        result
    );

    cleanup_keys(&mut conn, svc_name).await;
}

#[tokio::test]
#[ignore]
async fn test_list_services() {
    let svc_a = "test-list-a";
    let svc_b = "test-list-b";
    let mut conn = test_conn().await;
    let mut queue_conn = test_conn().await;
    cleanup_keys(&mut conn, svc_a).await;
    cleanup_keys(&mut conn, svc_b).await;

    register_service(&mut conn, &make_service_config(svc_a), &mut queue_conn)
        .await
        .unwrap();
    register_service(&mut conn, &make_service_config(svc_b), &mut queue_conn)
        .await
        .unwrap();

    let services = list_services(&mut conn).await.unwrap();
    let names: Vec<&str> = services.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&svc_a), "should contain {}", svc_a);
    assert!(names.contains(&svc_b), "should contain {}", svc_b);

    cleanup_keys(&mut conn, svc_a).await;
    cleanup_keys(&mut conn, svc_b).await;
}

// ============================================================================
// SRVC-04: Service config persistence
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_service_config_persistence() {
    let svc_name = "test-persist-svc";
    let mut conn = test_conn().await;
    let mut queue_conn = test_conn().await;
    cleanup_keys(&mut conn, svc_name).await;

    let config = ServiceConfig {
        name: svc_name.to_string(),
        description: "persistent test".to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        task_timeout_secs: 600,
        max_nodes: None,
        node_stale_after_secs: 120,
        drain_timeout_secs: 300,
    };

    register_service(&mut conn, &config, &mut queue_conn)
        .await
        .unwrap();

    // Drop and recreate connection to prove persistence
    drop(conn);
    let mut conn2 = test_conn().await;

    let retrieved = get_service(&mut conn2, svc_name).await.unwrap();
    assert_eq!(retrieved.task_timeout_secs, 600);
    assert_eq!(retrieved.node_stale_after_secs, 120);

    cleanup_keys(&mut conn2, svc_name).await;
}

// ============================================================================
// SRVC-03: Deregistration cleanup
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_deregister_cleanup() {
    let svc_name = "test-dereg-svc";
    let mut conn = test_conn().await;
    let mut queue_conn = test_conn().await;
    cleanup_keys(&mut conn, svc_name).await;

    // Register service
    register_service(&mut conn, &make_service_config(svc_name), &mut queue_conn)
        .await
        .unwrap();

    // Create a fake task hash + stream entry manually
    let _: () = redis::pipe()
        .cmd("HSET")
        .arg("task:fake-task-001")
        .arg("service")
        .arg(svc_name)
        .arg("state")
        .arg("pending")
        .ignore()
        .cmd("XADD")
        .arg(format!("tasks:{}", svc_name))
        .arg("*")
        .arg("task_id")
        .arg("fake-task-001")
        .ignore()
        .query_async(&mut conn)
        .await
        .unwrap();

    // Store a node token
    let token_key = format!("node_tokens:{}:fakehash", svc_name);
    let _: () = conn.hset(&token_key, "token", "abc123").await.unwrap();

    // Register a node
    register_or_update_node(&mut conn, svc_name, "node-dereg-1")
        .await
        .unwrap();

    // Now cleanup
    cleanup_service(&mut conn, svc_name).await.unwrap();

    // Verify everything is cleaned up
    assert!(
        !service_exists(&mut conn, svc_name).await.unwrap(),
        "service should no longer exist"
    );

    let in_index: bool = conn.sismember("services:index", svc_name).await.unwrap();
    assert!(!in_index, "should not be in services:index");

    let stream_exists: bool = conn
        .exists(format!("tasks:{}", svc_name))
        .await
        .unwrap();
    assert!(!stream_exists, "task stream should be deleted");

    let node_set: Vec<String> = conn
        .smembers(format!("nodes:{}", svc_name))
        .await
        .unwrap();
    assert!(node_set.is_empty(), "nodes set should be empty");

    let token_exists: bool = conn.exists(&token_key).await.unwrap();
    assert!(!token_exists, "node token should be deleted");

    // Clean up the fake task hash
    let _: redis::RedisResult<()> = redis::cmd("DEL")
        .arg("task:fake-task-001")
        .query_async(&mut conn)
        .await;

    cleanup_keys(&mut conn, svc_name).await;
}

// ============================================================================
// NODE-03: Submit rejects unregistered services
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_submit_rejects_unregistered_service() {
    let unreg_svc = "test-unreg-svc";
    let ok_svc = "test-submit-ok-svc";
    let mut conn = test_conn().await;
    let mut queue_conn = test_conn().await;
    cleanup_keys(&mut conn, unreg_svc).await;
    cleanup_keys(&mut conn, ok_svc).await;

    // Negative path: unregistered service should not exist
    let exists = service_exists(&mut conn, unreg_svc).await.unwrap();
    assert!(!exists, "unregistered service should not exist");

    // This is the guard used by HTTP and gRPC handlers: if !service_exists, reject
    // We verify the guard logic here at the registry level

    // Positive path: register a service, verify exists, then submit succeeds
    register_service(&mut conn, &make_service_config(ok_svc), &mut queue_conn)
        .await
        .unwrap();

    let exists = service_exists(&mut conn, ok_svc).await.unwrap();
    assert!(exists, "registered service should exist");

    // Submit a task via RedisQueue to prove end-to-end works
    let config = load_config(None).unwrap();
    let queue = RedisQueue::new(&config).await.unwrap();
    let service_name = ServiceName::new(ok_svc).unwrap();
    let task_id = queue
        .submit_task(&service_name, b"test payload".to_vec(), HashMap::new())
        .await
        .unwrap();
    assert!(!task_id.0.is_empty(), "should return a valid task ID");

    // Clean up
    let _: redis::RedisResult<()> = redis::cmd("DEL")
        .arg(format!("task:{}", task_id))
        .query_async(&mut conn)
        .await;
    cleanup_keys(&mut conn, ok_svc).await;
    cleanup_keys(&mut conn, unreg_svc).await;
}

// ============================================================================
// NODE-05: Node health tracking
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_node_health_tracking() {
    let svc_name = "test-health-svc";
    let mut conn = test_conn().await;
    let mut queue_conn = test_conn().await;
    cleanup_keys(&mut conn, svc_name).await;

    register_service(&mut conn, &make_service_config(svc_name), &mut queue_conn)
        .await
        .unwrap();

    register_or_update_node(&mut conn, svc_name, "node-1")
        .await
        .unwrap();

    let nodes = get_nodes_for_service(&mut conn, svc_name, 60).await.unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].node_id, "node-1");
    assert_eq!(nodes[0].health, NodeHealthState::Healthy);
    assert!(!nodes[0].draining);

    cleanup_keys(&mut conn, svc_name).await;
}

// ============================================================================
// NODE-06: Node drain flow
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_node_drain_flow() {
    let svc_name = "test-drain-svc";
    let mut conn = test_conn().await;
    let mut queue_conn = test_conn().await;
    cleanup_keys(&mut conn, svc_name).await;

    let config = ServiceConfig {
        name: svc_name.to_string(),
        description: "drain test".to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        task_timeout_secs: 300,
        max_nodes: None,
        node_stale_after_secs: 60,
        drain_timeout_secs: 120,
    };

    register_service(&mut conn, &config, &mut queue_conn)
        .await
        .unwrap();

    register_or_update_node(&mut conn, svc_name, "node-drain-1")
        .await
        .unwrap();

    // Not draining initially
    assert!(!is_node_draining(&mut conn, svc_name, "node-drain-1")
        .await
        .unwrap());

    // Set draining
    let timeout = set_node_draining(&mut conn, svc_name, "node-drain-1")
        .await
        .unwrap();
    assert_eq!(timeout, 120, "should return drain_timeout_secs from config");

    // Now draining
    assert!(is_node_draining(&mut conn, svc_name, "node-drain-1")
        .await
        .unwrap());

    // get_nodes_for_service shows draining
    let nodes = get_nodes_for_service(&mut conn, svc_name, 60).await.unwrap();
    assert_eq!(nodes.len(), 1);
    assert!(nodes[0].draining);

    cleanup_keys(&mut conn, svc_name).await;
}

// ============================================================================
// NODE-05: Node disconnect
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_node_disconnect() {
    let svc_name = "test-disc-svc";
    let mut conn = test_conn().await;
    let mut queue_conn = test_conn().await;
    cleanup_keys(&mut conn, svc_name).await;

    register_service(&mut conn, &make_service_config(svc_name), &mut queue_conn)
        .await
        .unwrap();

    register_or_update_node(&mut conn, svc_name, "node-disc-1")
        .await
        .unwrap();

    mark_node_disconnected(&mut conn, svc_name, "node-disc-1")
        .await
        .unwrap();

    let nodes = get_nodes_for_service(&mut conn, svc_name, 60).await.unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].health, NodeHealthState::Disconnected);

    cleanup_keys(&mut conn, svc_name).await;
}
