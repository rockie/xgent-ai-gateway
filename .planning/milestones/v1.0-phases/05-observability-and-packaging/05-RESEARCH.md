# Phase 5: Observability and Packaging - Research

**Researched:** 2026-03-22
**Domain:** Structured logging, Prometheus metrics, admin health API, static binary packaging, Docker image
**Confidence:** HIGH

## Summary

Phase 5 adds observability (structured JSON logs, Prometheus metrics, admin health endpoint) and production packaging (musl static binary, Docker image) to the gateway. The existing codebase already has `tracing` and `tracing-subscriber` (with `json` feature) in dependencies, admin auth patterns established, and node health data models ready for the health API. The `prometheus` crate (0.14.0) is the standard Rust Prometheus client and maps directly to the decided metric surface. `tikv-jemallocator` (0.6.1) provides the jemalloc allocator for musl targets with a straightforward cfg-gated setup.

The primary complexity is in the metrics instrumentation -- touching many files across the codebase to record counters and histograms at the right points. The logging upgrade is a localized change to `main.rs` subscriber initialization. The Docker build is a standard multi-stage pattern with no novel challenges.

**Primary recommendation:** Implement in order: (1) logging config + JSON subscriber, (2) prometheus metrics registration + `/metrics` endpoint, (3) instrument all code paths with metric recording, (4) admin health endpoint, (5) jemalloc + musl build, (6) Dockerfile + `.dockerignore`.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Always-present fields: `timestamp`, `level`, `message`, `target` (Rust module path), plus contextual `task_id`/`service`/`node_id` when available. No request_id or span_id -- minimal approach.
- **D-02:** Log level policy (conservative): ERROR = process-threatening (Redis disconnect, TLS failure), WARN = recoverable failures (callback delivery failed, auth rejected), INFO = lifecycle events (server start, task state transitions), DEBUG = per-request detail.
- **D-03:** JSON activation controlled via TOML config: `logging.format = "json" | "text"`. Default to `"text"` for dev ergonomics, production configs set `"json"`. Env var override via `GATEWAY__LOGGING__FORMAT`.
- **D-04:** Log output to stdout by default. Optional file output via `logging.file` config path for non-container deployments. When file is set, logs go to both stdout and file.
- **D-05:** Use the `prometheus` crate (official Prometheus client for Rust). `TextEncoder` for `/metrics` endpoint. Register metrics at startup, pass registry through `AppState`.
- **D-06:** Metrics surface -- 8 metrics covering queue depth, task duration, active nodes, errors, submissions, completions, callbacks, poll latency.
- **D-07:** Exponential histogram buckets: 0.1s, 0.25s, 0.5s, 1s, 2.5s, 5s, 10s, 30s, 60s, 120s, 300s.
- **D-08:** `/metrics` endpoint on HTTP listener, behind admin auth (`admin.token` guard).
- **D-09:** `GET /v1/admin/health` endpoint returning per-service node health data. Behind admin auth.
- **D-10:** jemalloc allocator via `tikv-jemallocator` with `#[global_allocator]`.
- **D-11:** Docker base image: `FROM alpine:3.19`. Includes CA certs, timezone data.
- **D-12:** Ship default `gateway.toml` embedded in image at `/etc/xgent/gateway.toml`.
- **D-13:** Multi-stage Dockerfile with `FROM rust:latest AS builder`. Build with musl target inside Docker.

### Claude's Discretion
- Exact `tracing-subscriber` layer composition for JSON + file output
- `prometheus` crate registry setup and metric registration pattern
- How to compute `queue_depth` gauge (Redis XLEN vs XPENDING count)
- How to compute `task_duration_seconds` (timestamp diff from task hash fields)
- `poll_latency` measurement approach (created_at vs claimed_at diff)
- Multi-stage Dockerfile exact structure (cargo-chef for layer caching vs plain build)
- `.dockerignore` contents
- OBSV-03 response JSON schema
- Whether jemalloc needs feature flags for musl compatibility
- Graceful handling of file logger errors (fallback to stdout-only)

### Deferred Ideas (OUT OF SCOPE)
- OpenTelemetry tracing with W3C trace context propagation -- v2 requirement (ADVF-03)
- Request-id header propagation
- Per-endpoint latency histograms for HTTP/gRPC handlers
- Metrics push gateway support
- Health check endpoint for load balancers (separate from admin health)
- Grafana dashboard JSON export
- Log rotation for file output
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| OBSV-01 | Gateway emits structured JSON logs with task ID, service, and node context in every log line | tracing-subscriber JSON layer with configurable format; `tracing::info!(task_id=..., service=..., ...)` structured fields |
| OBSV-02 | Gateway exposes Prometheus metrics endpoint (queue depth, task latency, node counts, error rates) | `prometheus` crate 0.14.0 with CounterVec, GaugeVec, HistogramVec; TextEncoder for `/metrics` response |
| OBSV-03 | Node health dashboard data available via admin API (active nodes, last seen, in-flight tasks) | Existing `get_nodes_for_service` in `node_health.rs` + `list_services` provides all data; wrap in new endpoint |
| INFR-03 | Gateway builds as a single static binary (musl target) | `tikv-jemallocator` 0.6.1 for musl allocator; `x86_64-unknown-linux-musl` target; build inside Docker for CI portability |
| INFR-04 | Gateway ships as a Docker image | Multi-stage Dockerfile: rust:latest builder -> alpine:3.19 runtime; embedded default config |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| prometheus | 0.14.0 | Prometheus metrics client | Official Rust Prometheus client by TiKV. TextEncoder produces standard exposition format. Counter, Gauge, Histogram types with Vec variants for labels. |
| tikv-jemallocator | 0.6.1 | jemalloc global allocator | Eliminates musl allocator performance regression (10x slowdown in multithreaded workloads). Cfg-gated for musl-only. Battle-tested by TiKV. |
| tracing-subscriber | 0.3.x (already in deps) | Structured log output | Already a dependency with `json` feature enabled. Layered subscriber supports JSON + file output composition. |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tracing-appender | 0.2.x | Non-blocking file writer | For D-04 file output. Provides `non_blocking` writer that avoids blocking the async runtime when writing to files. Ships as part of tracing ecosystem. |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| prometheus 0.14 | metrics + metrics-prometheus | `metrics` is a facade crate; adds indirection. `prometheus` is simpler and direct for this use case. |
| tracing-appender | std::fs::File | Would block the runtime. tracing-appender wraps in a non-blocking worker thread. |
| cargo-chef (Docker cache) | Plain cargo build | cargo-chef caches dependency compilation in a separate Docker layer. Faster rebuilds but adds complexity. For a 2-crate workspace, the benefit is moderate. |

**Installation (additions to gateway/Cargo.toml):**
```toml
prometheus = "0.14"
tracing-appender = "0.2"

[target.'cfg(target_env = "musl")'.dependencies]
tikv-jemallocator = "0.6"
```

## Architecture Patterns

### Recommended Project Structure (additions)
```
gateway/src/
  config.rs          # Add LoggingConfig section
  state.rs           # Add Metrics struct to AppState
  metrics.rs         # NEW: metric definitions, registration, periodic gauge refresh
  http/admin.rs      # Add /v1/admin/health and /metrics endpoints
  main.rs            # Upgrade tracing subscriber, register metrics
```

### Pattern 1: Metrics Struct in AppState
**What:** Define a `Metrics` struct that holds all pre-registered metric handles (CounterVec, GaugeVec, HistogramVec). Register at startup, store in `AppState`, access from handlers.
**When to use:** Always -- avoids global statics, testable, explicit ownership.
**Example:**
```rust
// Source: prometheus crate docs + project pattern
use prometheus::{CounterVec, GaugeVec, HistogramVec, HistogramOpts, Opts, Registry};

pub struct Metrics {
    pub registry: Registry,
    pub tasks_submitted: CounterVec,
    pub tasks_completed: CounterVec,
    pub errors_total: CounterVec,
    pub callback_delivery: CounterVec,
    pub queue_depth: GaugeVec,
    pub nodes_active: GaugeVec,
    pub task_duration: HistogramVec,
    pub poll_latency: HistogramVec,
}

impl Metrics {
    pub fn new() -> Self {
        let registry = Registry::new();
        let buckets = vec![0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0];

        let tasks_submitted = CounterVec::new(
            Opts::new("gateway_tasks_submitted_total", "Tasks submitted"),
            &["service", "protocol"],
        ).unwrap();

        let task_duration = HistogramVec::new(
            HistogramOpts::new("gateway_task_duration_seconds", "Task lifecycle duration")
                .buckets(buckets.clone()),
            &["service", "status"],
        ).unwrap();

        // ... register all with registry
        registry.register(Box::new(tasks_submitted.clone())).unwrap();
        registry.register(Box::new(task_duration.clone())).unwrap();

        Self { registry, tasks_submitted, task_duration, /* ... */ }
    }
}
```

### Pattern 2: Layered tracing-subscriber for JSON + File
**What:** Use `tracing_subscriber::registry()` with composed layers for stdout (JSON or text) and optional file output.
**When to use:** D-03 and D-04 -- switching format by config and dual output.
**Example:**
```rust
// Source: tracing-subscriber docs
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

fn init_tracing(config: &LoggingConfig) {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let is_json = config.format == "json";

    // Stdout layer -- JSON or text
    let stdout_layer = fmt::layer()
        .with_target(true);

    // File layer (optional)
    let file_layer = config.file.as_ref().map(|path| {
        let file = std::fs::OpenOptions::new()
            .create(true).append(true).open(path).expect("open log file");
        let (non_blocking, _guard) = tracing_appender::non_blocking(file);
        fmt::layer().json().with_writer(non_blocking)
    });

    // Compose: registry + filter + stdout + optional file
    if is_json {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(stdout_layer.json())
            .with(file_layer)
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(stdout_layer)
            .with(file_layer)
            .init();
    }
}
```
**Note:** The `_guard` from `non_blocking()` must be held for the process lifetime. Return it from init and store in main.

### Pattern 3: Periodic Gauge Refresh via Background Task
**What:** Queue depth and active nodes are gauges that must be periodically refreshed from Redis. Spawn a background task that queries Redis and updates gauge values on a timer.
**When to use:** For `gateway_queue_depth` and `gateway_nodes_active` gauges.
**Example:**
```rust
// Spawn alongside the reaper
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(15));
    loop {
        interval.tick().await;
        // For each service: XLEN tasks:{service} -> queue_depth gauge
        // For each service: count healthy nodes -> nodes_active gauge
    }
});
```
**Recommendation for queue_depth:** Use `XLEN tasks:{service}` for total pending items in the stream. This gives the pending queue size (items not yet consumed). XPENDING gives claimed-but-unacked which is a different metric (in-flight, not queued).

### Pattern 4: /metrics Endpoint with TextEncoder
**What:** Axum handler that gathers metrics from the registry and returns Prometheus text format.
**Example:**
```rust
pub async fn metrics_handler(
    State(state): State<Arc<AppState>>,
) -> impl axum::response::IntoResponse {
    let encoder = prometheus::TextEncoder::new();
    let metric_families = state.metrics.registry.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    (
        [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        buffer,
    )
}
```

### Anti-Patterns to Avoid
- **Global static metrics with lazy_static:** While common in examples, this makes testing harder and hides dependencies. Use the Metrics struct in AppState instead.
- **Blocking file writes in async context:** Never use `std::fs::File` directly as a tracing writer. Use `tracing-appender::non_blocking` to wrap file I/O.
- **Computing gauges on scrape:** Don't query Redis inside the `/metrics` handler. Use a background refresh task. The scrape handler must be fast and non-blocking.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Prometheus text format | Custom string formatting | `prometheus::TextEncoder` | Format spec has subtle rules (escaping, ordering, metadata lines). TextEncoder handles all edge cases. |
| Non-blocking file logging | Custom thread + channel | `tracing-appender::non_blocking` | Handles backpressure, graceful shutdown, buffer flushing. |
| Histogram bucket math | Manual percentile tracking | `prometheus::Histogram` with custom buckets | Prometheus histograms use cumulative bucket counters -- the math is non-trivial to get right. |
| Docker layer caching | Manual COPY ordering | cargo-chef (optional) or careful COPY Cargo.toml first | Docker rebuilds all subsequent layers when a layer changes. Separating dependency compilation from source compilation saves rebuild time. |

**Key insight:** The prometheus crate handles all the exposition format complexity. The tracing ecosystem handles all async-safe logging complexity. The only custom code needed is wiring: which metric to increment where, and what config fields to add.

## Common Pitfalls

### Pitfall 1: tracing-appender Guard Dropped Early
**What goes wrong:** Log file output silently stops because the `WorkerGuard` returned by `non_blocking()` is dropped.
**Why it happens:** The guard is created in a function scope and not returned/stored.
**How to avoid:** Return the guard from `init_tracing()` and bind it in `main()`: `let _guard = init_tracing(&config);`
**Warning signs:** Log file stops growing while stdout logging continues.

### Pitfall 2: Prometheus Metric Not Registered
**What goes wrong:** Metric appears to work (no panic on `.inc()`) but never shows up in `/metrics` output.
**Why it happens:** Metric was created but not registered with the `Registry` via `registry.register(Box::new(metric.clone()))`.
**How to avoid:** Register all metrics in the `Metrics::new()` constructor. Add a test that gathers from the registry and checks metric names.
**Warning signs:** `/metrics` endpoint returns output but specific metrics are missing.

### Pitfall 3: Label Cardinality Explosion
**What goes wrong:** Prometheus scrapes become slow, memory usage grows unbounded.
**Why it happens:** Using high-cardinality values (task IDs, node IDs) as metric labels.
**How to avoid:** Only use bounded label values: `service` (finite registered services), `protocol` ("grpc"/"http"), `status` ("success"/"failure"), `type` (error categories). Never use task_id or node_id as labels.
**Warning signs:** `/metrics` output grows linearly with request count.

### Pitfall 4: musl Allocator Performance Regression
**What goes wrong:** Gateway performance drops 10x under concurrent load when built with musl.
**Why it happens:** musl's default allocator uses a global lock, causing severe contention with Tokio's multi-threaded runtime.
**How to avoid:** Use `tikv-jemallocator` with `#[global_allocator]`, cfg-gated to musl targets only.
**Warning signs:** High CPU usage with low throughput, increased tail latencies.

### Pitfall 5: Docker COPY Invalidates Layer Cache
**What goes wrong:** Every code change triggers a full `cargo build` in Docker, making builds take 5-10 minutes.
**Why it happens:** `COPY . .` before `cargo build` means any source change invalidates the dependency compilation layer.
**How to avoid:** Copy `Cargo.toml` and `Cargo.lock` first, run `cargo build` to cache deps, then copy source and build again. Or use `cargo-chef`.
**Warning signs:** Docker builds are consistently slow even for trivial changes.

### Pitfall 6: JSON Format Branch in Subscriber Init
**What goes wrong:** Code duplication or type errors when conditionally choosing JSON vs text format.
**Why it happens:** `fmt::layer().json()` returns a different type than `fmt::layer()`. Can't assign both to the same variable.
**How to avoid:** Use separate `if/else` branches that each call `.init()`, or use `Option<Layer>` composition (tracing-subscriber supports `Option<L>` where `None` is a no-op layer).
**Warning signs:** Complex generic bounds or `Box<dyn Layer>` gymnastics.

## Code Examples

### LoggingConfig Addition to config.rs
```rust
// Follows existing config.rs pattern with #[serde(default)]
#[derive(Debug, Deserialize, Clone)]
pub struct LoggingConfig {
    /// Log output format: "json" or "text". Default: "text".
    #[serde(default = "default_log_format")]
    pub format: String,
    /// Optional file path for log output. When set, logs go to both stdout and file.
    pub file: Option<String>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            format: default_log_format(),
            file: None,
        }
    }
}

fn default_log_format() -> String {
    "text".to_string()
}
```

### jemalloc Setup in main.rs
```rust
// Source: tikv-jemallocator docs + raniz.blog musl performance article
#[cfg(target_env = "musl")]
use tikv_jemallocator::Jemalloc;

#[cfg(target_env = "musl")]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;
```

### Admin Health Endpoint Response Schema (OBSV-03)
```rust
#[derive(Serialize)]
pub struct HealthResponse {
    pub services: Vec<ServiceHealth>,
}

#[derive(Serialize)]
pub struct ServiceHealth {
    pub name: String,
    pub active_nodes: u32,    // count of healthy nodes
    pub total_nodes: u32,     // total registered nodes
    pub nodes: Vec<NodeHealth>,
}

#[derive(Serialize)]
pub struct NodeHealth {
    pub node_id: String,
    pub health: String,       // "healthy" | "unhealthy" | "disconnected"
    pub last_seen: String,    // RFC3339 timestamp
    pub in_flight_tasks: u32,
}
```

### Metric Instrumentation Points
```rust
// In http/submit.rs or grpc/submit.rs after successful submission:
state.metrics.tasks_submitted
    .with_label_values(&[&service_name, "http"]) // or "grpc"
    .inc();

// In queue/redis.rs report_result after terminal state:
let duration = compute_duration(&created_at, &completed_at);
state.metrics.task_duration
    .with_label_values(&[&service, &status])
    .observe(duration);

// In callback/mod.rs after delivery attempt:
state.metrics.callback_delivery
    .with_label_values(&[if success { "success" } else { "failure" }])
    .inc();
```

### task_duration Computation
```rust
// Compute duration from task hash timestamps (created_at -> completed_at)
fn compute_task_duration_secs(created_at: &str, completed_at: &str) -> Option<f64> {
    let created = chrono::DateTime::parse_from_rfc3339(created_at).ok()?;
    let completed = chrono::DateTime::parse_from_rfc3339(completed_at).ok()?;
    let duration = completed.signed_duration_since(created);
    Some(duration.num_milliseconds() as f64 / 1000.0)
}
```

### poll_latency Computation
```rust
// Compute poll latency: time from task creation to node claiming it
// created_at is stored in task hash at submit time
// claimed_at = now (when the node picks up the task via XREADGROUP)
fn compute_poll_latency_secs(created_at: &str) -> Option<f64> {
    let created = chrono::DateTime::parse_from_rfc3339(created_at).ok()?;
    let now = chrono::Utc::now();
    let duration = now.signed_duration_since(created);
    Some(duration.num_milliseconds() as f64 / 1000.0)
}
```

### Dockerfile Multi-Stage Build
```dockerfile
# Stage 1: Build
FROM rust:latest AS builder

RUN rustup target add x86_64-unknown-linux-musl
RUN apt-get update && apt-get install -y musl-tools protobuf-compiler

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY proto/Cargo.toml proto/
COPY gateway/Cargo.toml gateway/

# Create dummy source files to cache dependency compilation
RUN mkdir -p proto/src gateway/src && \
    echo "fn main() {}" > gateway/src/main.rs && \
    echo "" > gateway/src/lib.rs && \
    echo "" > proto/src/lib.rs && \
    touch proto/build.rs

# Build dependencies only (cached layer)
RUN cargo build --release --target x86_64-unknown-linux-musl || true

# Copy real source
COPY . .
# Touch to invalidate cached dummy sources
RUN touch gateway/src/main.rs proto/src/lib.rs proto/build.rs

RUN cargo build --release --target x86_64-unknown-linux-musl

# Stage 2: Runtime
FROM alpine:3.19

RUN apk add --no-cache ca-certificates tzdata

COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/xgent-gateway /usr/local/bin/
COPY gateway.toml /etc/xgent/gateway.toml

EXPOSE 8080 50051

ENTRYPOINT ["xgent-gateway"]
CMD ["--config", "/etc/xgent/gateway.toml"]
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `lazy_static!` for metrics | Metrics struct in app state | 2024+ ecosystem shift | Testable, no hidden global state |
| `prometheus` 0.13.x | `prometheus` 0.14.0 | 2024 | Minor API updates, same patterns |
| `tracing-subscriber` plain init | Layered registry composition | Always available, better documented now | Supports conditional layers cleanly |
| `FROM scratch` Docker | `FROM alpine:3.19` | Project decision | Adds ~7MB but provides debugging shell and CA certs |
| mimalloc for musl | jemalloc (tikv-jemallocator) | Project decision D-10 | jemalloc is what Redis itself uses; well-suited for Redis-backed gateway |

**Deprecated/outdated:**
- `slog`: Legacy structured logging. `tracing` is the standard.
- `prometheus` default registry (`prometheus::default_registry()`): Works but is global. Prefer explicit `Registry::new()`.

## Open Questions

1. **cargo-chef vs plain dependency caching in Dockerfile**
   - What we know: cargo-chef provides more reliable dependency caching. Plain dummy-source approach works but can be fragile.
   - What's unclear: Whether the 2-crate workspace benefits enough to justify the extra build step.
   - Recommendation: Start with plain dependency caching pattern (shown above). Add cargo-chef later if build times become a problem.

2. **queue_depth gauge: XLEN vs XPENDING**
   - What we know: XLEN gives total entries in stream. XPENDING gives claimed-but-unacked entries. The true "pending" count for tasks waiting to be picked up is approximately XLEN minus XPENDING count.
   - What's unclear: Stream trimming (MAXLEN ~) means XLEN may not perfectly reflect pending task count.
   - Recommendation: Use `XLEN` for the gauge -- it represents "items in queue" which is the most useful operational signal. Document that this includes items being processed. Operators can subtract in-flight from node data for a "waiting" metric if needed.

3. **Admin auth for /metrics endpoint**
   - What we know: D-08 says behind admin auth. Existing admin routes in Phase 3 use `admin.token` config.
   - What's unclear: The current admin routes don't appear to have auth middleware applied (comment in main.rs says "unauthenticated in Phase 2").
   - Recommendation: Check if admin auth was added in Phase 3 as planned. If so, apply same pattern to `/metrics`. If not, add admin auth middleware that checks bearer token against `config.admin.token`.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test framework (cargo test) |
| Config file | None needed -- `#[cfg(test)]` modules + integration tests in `gateway/tests/` |
| Quick run command | `cargo test -p xgent-gateway` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| OBSV-01 | Structured JSON log output with contextual fields | unit | `cargo test -p xgent-gateway -- logging` | No -- Wave 0 |
| OBSV-02 | Prometheus metrics registered and exposed | unit + integration | `cargo test -p xgent-gateway -- metrics` | No -- Wave 0 |
| OBSV-03 | Admin health endpoint returns node data | integration | `cargo test -p xgent-gateway --test integration_test -- health` | No -- Wave 0 |
| INFR-03 | Static binary builds for musl target | build verification | `cargo build --release --target x86_64-unknown-linux-musl` | N/A (CI build step) |
| INFR-04 | Docker image builds and runs | build verification | `docker build -t xgent-gateway .` | N/A (CI build step) |

### Sampling Rate
- **Per task commit:** `cargo test -p xgent-gateway`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green + `cargo build --release --target x86_64-unknown-linux-musl` succeeds

### Wave 0 Gaps
- [ ] `gateway/src/metrics.rs` -- unit tests for Metrics struct creation, registration, and TextEncoder output
- [ ] Config test for `LoggingConfig` default values and TOML/env override in `config.rs` tests
- [ ] OBSV-02 test: create Metrics, increment counters, gather and verify expected metric names in output
- [ ] OBSV-03 test: admin health endpoint returns expected JSON schema (integration test)

## Sources

### Primary (HIGH confidence)
- [prometheus 0.14.0 docs](https://docs.rs/prometheus/0.14.0/prometheus/) -- API for Registry, Counter, Gauge, Histogram, TextEncoder
- [tikv-jemallocator on crates.io](https://crates.io/crates/tikv-jemallocator) -- version 0.6.1 verified
- [tracing-subscriber docs](https://docs.rs/tracing-subscriber) -- JSON layer, registry composition, Option<Layer> support
- [Raniz blog: Rust MUSL malloc performance](https://raniz.blog/2025-02-06_rust-musl-malloc/) -- 10x musl allocator regression, jemalloc fix confirmed

### Secondary (MEDIUM confidence)
- [tikv/jemallocator GitHub](https://github.com/tikv/jemallocator) -- musl target configuration pattern, cfg-gated dependency
- [tracing-subscriber multiple layers forum](https://users.rust-lang.org/t/type-hell-in-tracing-multiple-output-layers/126764) -- community patterns for multi-layer composition

### Tertiary (LOW confidence)
- Docker multi-stage build pattern -- based on established Rust Docker practices, not verified against specific recent changes

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- prometheus 0.14.0, tikv-jemallocator 0.6.1 verified on crates.io. tracing-subscriber already in deps.
- Architecture: HIGH -- patterns follow existing codebase conventions (AppState, config sections). Metrics struct pattern is well-established.
- Pitfalls: HIGH -- musl allocator regression is well-documented. tracing-appender guard is a common gotcha. Label cardinality is a known Prometheus issue.
- Packaging: MEDIUM -- Dockerfile pattern is standard but specific proto compilation in Docker may need adjustment.

**Research date:** 2026-03-22
**Valid until:** 2026-04-22 (stable domain, 30-day validity)
