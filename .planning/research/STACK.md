# Stack Research

**Domain:** Rust pull-model task gateway (gRPC + HTTPS, Redis-backed queues)
**Researched:** 2026-03-21
**Confidence:** HIGH

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| **Rust** | stable (1.85+) | Language | Performance, memory safety, excellent async ecosystem. Constraint from PROJECT.md. |
| **Tokio** | 1.43+ (LTS) | Async runtime | The async runtime for Rust. Tokio 1.43 is LTS until March 2026, 1.47 LTS until Sep 2026. Every library in this stack builds on Tokio. No alternative worth considering. |
| **Tonic** | 0.14.x | gRPC server/client | The Rust gRPC implementation. Built on Tokio + Hyper + Prost. Mature, actively maintained, supports streaming, interceptors, mTLS via rustls. |
| **Axum** | 0.8.x | HTTP server | Tokio-team's HTTP framework. Shares the Hyper + Tower foundation with Tonic, enabling co-hosting gRPC and HTTP on the same port. Tower middleware works across both. |
| **Hyper** | 1.x | HTTP/2 transport | Underlying HTTP engine for both Axum and Tonic. Not used directly but pulled in as a dependency. Enables the single-port multiplexing pattern. |
| **Redis (redis-rs)** | 1.0.x | Redis/Valkey client | The standard Rust Redis client, now at 1.0. Supports async via `tokio-comp` feature, `MultiplexedConnection` (clone-safe, cancellation-safe), RESP3, and Valkey compatibility. |
| **Prost** | 0.14.x | Protobuf codegen | Protocol Buffers for Rust. Used by Tonic for gRPC message serialization. Tonic-build 0.14.x drives codegen from `.proto` files. |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| **rustls** | 0.23.x | TLS implementation | All TLS needs: HTTPS termination, mTLS for gRPC clients, TLS to Redis. Pure Rust -- no OpenSSL dependency means easier static linking and cross-compilation. |
| **rcgen** | 0.13.x | Certificate generation | Dev/test only: generate self-signed certs and CA chains for mTLS testing. Do not use in production cert management. |
| **serde** | 1.x | Serialization | JSON request/response bodies on the HTTP side, config file parsing, Redis value serialization. Ubiquitous in Rust -- use `serde_json` for JSON. |
| **tracing** | 0.1.x | Structured logging | Async-aware structured logging. Use `tracing-subscriber` for output formatting, `tracing-opentelemetry` if observability backends needed later. |
| **clap** | 4.6.x | CLI argument parsing | Parse `--config`, `--port`, `--redis-url` flags. Use the derive API (`#[derive(Parser)]`) for type-safe argument definitions. |
| **tower** | 0.5.x | Middleware framework | Shared middleware layer between Axum and Tonic: timeouts, rate limiting, auth extraction, request tracing. Both frameworks are Tower-native. |
| **tower-http** | 0.6.x | HTTP middleware | CORS, compression, request-id, sensitive headers. Axum-specific Tower layers for the HTTP side. |
| **uuid** | 1.x | Task IDs | Generate unique task identifiers. Use `v7` feature for time-sortable UUIDs (useful for debugging chronological task order). |
| **chrono** | 0.4.x | Timestamps | Task lifecycle timestamps (created, assigned, completed). Serde integration for Redis storage. |
| **tokio-rustls** | 0.26.x | Async TLS | Bridges rustls into Tokio's async I/O. Required for mTLS on the gRPC listener and TLS to Redis/Valkey. |
| **config** | 0.15.x | Configuration | Layered config: defaults -> config file -> env vars -> CLI args. TOML file support for gateway configuration. |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| **cargo** | Build system | Use workspaces if splitting into `gateway`, `proto`, `common` crates |
| **protoc** | Protobuf compiler | Required by `tonic-build`. Install via system package manager or `protobuf-src` crate for hermetic builds |
| **cargo-watch** | Dev reload | `cargo watch -x run` for auto-rebuild during development |
| **grpcurl** | gRPC testing | CLI tool for ad-hoc gRPC calls during development. Install via `brew install grpcurl` |
| **cross** | Cross-compilation | For building `x86_64-unknown-linux-musl` static binaries from macOS. Simpler than manual musl toolchain setup |
| **cargo-deny** | Dependency audit | License checking and vulnerability scanning in CI |

## Installation

```toml
# Cargo.toml

[dependencies]
# Core
tokio = { version = "1.43", features = ["full"] }
tonic = { version = "0.14", features = ["tls"] }
axum = "0.8"
hyper = { version = "1", features = ["http2", "server"] }
prost = "0.14"

# Redis
redis = { version = "1.0", features = ["tokio-comp", "aio"] }

# TLS / Auth
rustls = "0.23"
tokio-rustls = "0.26"
rcgen = "0.13"  # dev only

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Observability
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# Utilities
clap = { version = "4.6", features = ["derive"] }
uuid = { version = "1", features = ["v7", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
tower = { version = "0.5", features = ["timeout", "limit"] }
tower-http = { version = "0.6", features = ["cors", "trace", "compression-gzip"] }
config = "0.15"
thiserror = "2"
anyhow = "1"

[build-dependencies]
tonic-build = "0.14"
```

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| **Axum** (HTTP) | Actix-Web | Never for this project. Actix uses its own runtime (not Tokio-native), making Tower middleware sharing with Tonic impossible. Actix has slightly higher raw throughput but the interop cost is not worth it. |
| **Tonic** (gRPC) | grpc-rs (C-based) | Never. grpc-rs wraps the C gRPC core, adding C dependency complexity. Tonic is pure Rust, Tokio-native, and the clear community standard. |
| **redis-rs 1.0** (Redis) | fred 10.x | If you need built-in client-side clustering, reconnect policies, or round-robin pooling out of the box. Fred is more opinionated and feature-rich. redis-rs 1.0 is simpler and sufficient for this gateway's needs (single Redis instance, MultiplexedConnection handles concurrency). |
| **redis-rs MultiplexedConnection** | deadpool-redis / bb8-redis | If using older redis-rs (<1.0). With redis-rs 1.0, `MultiplexedConnection` is clone-safe and cancellation-safe -- a single connection multiplexes across tasks. No external pool crate needed for most workloads. Add a second connection for blocking operations (BRPOP) if using Redis as a blocking queue. |
| **rustls** (TLS) | OpenSSL via native-tls | Only if you must interop with legacy systems requiring specific OpenSSL cipher suites. rustls is safer (no C code), simpler to cross-compile, and sufficient for all standard TLS/mTLS needs. |
| **Prost** (protobuf) | protobuf-rs (stepancheg) | Never. Prost is the Tonic-native protobuf implementation. Using anything else with Tonic requires unnecessary adapter code. |
| **TOML config** | YAML / JSON config | Personal preference. TOML is Rust-idiomatic (Cargo.toml), simpler than YAML, and the `config` crate supports it natively. |
| **uuid v7** (task IDs) | ULID / nanoid | ULIDs if you need lexicographic sorting. uuid v7 provides time-ordering with broader ecosystem support. nanoid if you need shorter human-readable IDs (but lose time-ordering). |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| **Actix-Web** with Tonic | Different async runtime ecosystem. Tower middleware cannot be shared. You end up maintaining two middleware stacks. | Axum -- same Tokio/Tower/Hyper foundation as Tonic |
| **Warp** | Effectively unmaintained. Last meaningful update was 2023. Axum superseded it within the Tokio ecosystem. | Axum |
| **deadpool-redis** with redis 1.0 | Historical compatibility issues between deadpool-redis and newer redis-rs versions. redis-rs 1.0 MultiplexedConnection eliminates the need for external pooling in most cases. | `redis::aio::MultiplexedConnection` (built into redis-rs 1.0) |
| **OpenSSL / native-tls** | Adds C dependency, complicates static binary builds, larger attack surface. mTLS with OpenSSL is harder to configure correctly. | rustls |
| **async-std** | Tokio is the runtime for this stack. Mixing runtimes causes subtle bugs and doubled dependencies. | Tokio (already chosen) |
| **protobuf-rs** (stepancheg) | Incompatible with Tonic codegen. Would require manual adapter layer. | Prost (Tonic's native protobuf) |
| **slog** | Legacy structured logging. tracing is the modern standard with async-aware span tracking. | tracing |
| **rocket** | Historically used its own runtime. Rocket 0.5+ supports Tokio but still uses its own abstractions that don't interop with Tower. | Axum |

## Stack Patterns by Variant

**If co-hosting gRPC + HTTP on the same port (recommended):**
- Use Hyper as the underlying server, inspect `content-type: application/grpc` header to route to Tonic vs Axum
- Both Axum and Tonic implement `tower::Service<http::Request<Body>>`, so a multiplexer service switches between them
- This is a well-documented pattern: see `tonic` examples and `axum-tonic` crate
- Single port simplifies deployment, TLS termination, and load balancer config

**If using separate ports for gRPC and HTTP:**
- Simpler initial setup: two `tokio::spawn` calls, one for each listener
- Separate TLS configs (mTLS on gRPC port, standard TLS on HTTP port)
- Consider this if auth models diverge significantly between protocols
- Downside: two ports to manage, two health check endpoints

**If Redis queue uses blocking pops (BRPOP) for node polling:**
- Use a dedicated `MultiplexedConnection` for blocking operations
- The main connection handles non-blocking commands (SET, GET, LPUSH)
- BRPOP blocks the connection -- multiplexing still works but latency increases for other commands on the same connection
- Alternative: use Redis pub/sub to notify nodes, then non-blocking LPOP to dequeue

**If targeting scratch Docker images (smallest possible):**
- Build with `x86_64-unknown-linux-musl` target for fully static binary
- Use `cross` crate for cross-compilation from macOS
- Replace default musl allocator with `mimalloc` or `jemalloc` for better performance
- Final image: `FROM scratch` + binary + TLS certs only (~15-25MB)

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| tonic 0.14.x | prost 0.14.x, tonic-build 0.14.x | Tonic and Prost versions must match (same minor). Tonic 0.14 requires Prost 0.14. |
| tonic 0.14.x | axum 0.8.x | Both built on hyper 1.x and tower 0.5.x. Co-hosting verified and documented. |
| axum 0.8.x | tower 0.5.x, tower-http 0.6.x | Axum 0.8 requires tower 0.5+. tower-http 0.6.x targets tower 0.5. |
| redis 1.0.x | tokio 1.x | Use `tokio-comp` feature flag. MultiplexedConnection requires tokio runtime. |
| rustls 0.23.x | tokio-rustls 0.26.x | rustls 0.23 pairs with tokio-rustls 0.26. Do not mix with older tokio-rustls versions. |
| tonic 0.14.x (TLS) | rustls 0.23.x | Tonic's `tls` feature uses rustls internally. Verify tonic's rustls version matches your direct rustls dependency. |
| tokio 1.43+ | All above | LTS until March 2026. All crates in this stack target tokio 1.x. |

## Confidence Assessment

| Area | Confidence | Reasoning |
|------|------------|-----------|
| Tokio + Axum + Tonic | HIGH | De facto Rust async web/gRPC stack. Official Tokio team projects. Verified versions via crates.io and official announcements. |
| redis-rs 1.0 | HIGH | Verified 1.0.4 on crates.io. MultiplexedConnection documented in official docs. |
| No deadpool needed | MEDIUM | redis-rs 1.0 MultiplexedConnection should suffice, but under heavy load (thousands of tasks/hour) real benchmarking needed. If connection becomes bottleneck, add a second MultiplexedConnection rather than a pool crate. |
| rustls for mTLS | HIGH | Well-documented mTLS support. Tonic examples demonstrate rustls-based mTLS. |
| Co-hosting pattern | HIGH | Multiple documented examples, dedicated crate (axum-tonic), and official tonic examples show this pattern. |
| Static binary with musl | MEDIUM | Standard practice but rustls + musl sometimes has edge cases with certificate loading. Test early. |

## Sources

- [tonic on crates.io](https://crates.io/crates/tonic) -- version 0.14.5 verified
- [Axum 0.8.0 announcement](https://tokio.rs/blog/2025-01-01-announcing-axum-0-8-0) -- axum 0.8.x confirmed
- [Tokio releases](https://crates.io/crates/tokio) -- LTS 1.43 (March 2026), 1.47 (Sep 2026)
- [redis-rs on crates.io](https://crates.io/crates/redis) -- version 1.0.4 confirmed
- [redis-rs guide (official Redis docs)](https://redis.io/docs/latest/develop/clients/rust/) -- MultiplexedConnection usage
- [prost on crates.io](https://crates.io/crates/prost) -- version 0.14.2 confirmed
- [rustls on docs.rs](https://docs.rs/crate/rustls/latest) -- version 0.23.36 confirmed
- [rcgen releases](https://github.com/rustls/rcgen/releases) -- 0.13.x/0.14.x line confirmed
- [clap on crates.io](https://crates.io/crates/clap) -- version 4.6.x confirmed
- [HTTP+gRPC co-hosting](https://github.com/sunsided/http-grpc-cohosting) -- pattern reference
- [Axum+Tonic integration guide](https://dev.to/generatecodedev/how-to-run-axum-and-tonic-on-the-same-port-with-routing-4okk) -- co-hosting implementation
- [rust-musl-cross](https://github.com/rust-cross/rust-musl-cross) -- static binary compilation
- [deadpool-redis compatibility issues](https://noos.blog/posts/redis-tls-deadpool-compatibility/) -- rationale for avoiding deadpool

---
*Stack research for: Rust pull-model task gateway*
*Researched: 2026-03-21*
