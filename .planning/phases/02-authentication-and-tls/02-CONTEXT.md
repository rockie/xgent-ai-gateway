# Phase 2: Authentication and TLS - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

All connections to the gateway are authenticated and encrypted. HTTPS clients use API keys, gRPC clients use mTLS, internal nodes use per-service tokens, and all traffic runs over TLS with HTTP/2 keepalive pings. This phase layers security onto the existing dual-port Phase 1 infrastructure without changing the task flow.

</domain>

<decisions>
## Implementation Decisions

### API key storage and lookup
- **D-01:** API keys stored in a Redis hash, keyed by SHA-256 hash of the key
- **D-02:** Each key maps to client metadata including authorized service names (per-service scoping)
- **D-03:** Gateway hashes incoming key with SHA-256 and looks up the hash in Redis — raw keys never stored
- **D-04:** Consistent with Phase 1's Redis-for-everything pattern; keys survive restarts

### API key transport
- **D-05:** Accept API keys via both `Authorization: Bearer <key>` header and `X-API-Key: <key>` header
- **D-06:** If both headers are present, prefer `Authorization: Bearer`

### API key provisioning
- **D-07:** Admin API endpoint (POST /v1/admin/api-keys) to create and revoke keys
- **D-08:** Gateway generates the key, returns it once on creation — cannot be retrieved again
- **D-09:** Key creation requires specifying which services the key is authorized for

### API key error handling
- **D-10:** Always return generic `401 Unauthorized` with no detail about whether key was missing, invalid, expired, or unauthorized for the service
- **D-11:** Log the specific failure reason server-side for debugging (at debug/trace level)

### Claude's Discretion
- mTLS certificate handling: CA trust chain setup, client cert validation with rustls, rcgen for dev/test certs
- Node token design: token format, per-service scoping mechanism, storage in Redis, validation on each poll
- TLS configuration: cert/key file paths in config, separate TLS configs per port, rustls ServerConfig setup
- HTTP/2 keepalive: ping interval, timeout values, tonic and hyper keepalive configuration
- Admin API authentication (how to secure the admin endpoints themselves)
- Tower middleware vs Axum extractors for auth layer placement
- Whether to add TLS to Redis connection (rediss://) in this phase or defer

</decisions>

<specifics>
## Specific Ideas

- API key hash lookup should be fast — single Redis HGET per request, no multi-step lookups
- Per-service scoping means a client with key for service "inference" cannot submit to service "training" — enforced at the auth middleware layer before the handler runs
- The admin API for key management is a new route group (/v1/admin/*) that will need its own auth in Phase 3 or later

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project specifications
- `.planning/PROJECT.md` — Core constraints: Rust, dual protocol, Redis, auth model (API key for HTTPS, mTLS for gRPC, pre-shared tokens for nodes)
- `.planning/REQUIREMENTS.md` — AUTH-01 (API key), AUTH-02 (mTLS), AUTH-03 (node tokens), INFR-05 (TLS termination), INFR-06 (HTTP/2 keepalive)
- `.planning/ROADMAP.md` — Phase 2 success criteria (4 items)

### Technology stack
- `CLAUDE.md` §Technology Stack — rustls 0.23.x, tokio-rustls 0.26.x, rcgen 0.13.x for dev certs
- `CLAUDE.md` §Version Compatibility — rustls/tokio-rustls pairing, tonic TLS feature
- `CLAUDE.md` §What NOT to Use — No OpenSSL/native-tls (rustls only)

### Prior phase context
- `.planning/phases/01-core-queue-loop/01-CONTEXT.md` — Phase 1 decisions: dual-port hosting (D-05..D-07), project structure (D-08..D-09), existing patterns

### Blockers from STATE.md
- `.planning/STATE.md` §Blockers/Concerns — Static musl + rustls edge cases with certificate loading

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `gateway/src/config.rs` — GatewayConfig with GrpcConfig/HttpConfig structs, extensible for TLS fields (cert_path, key_path, etc.)
- `gateway/src/state.rs` — AppState holds queue + config; can be extended with auth state (key store, TLS config)
- `gateway/src/error.rs` — GatewayError enum for consistent error handling across gRPC and HTTP

### Established Patterns
- Dual-port pattern: separate `tokio::spawn` for gRPC and HTTP listeners in `main.rs` — TLS wraps each independently
- `Arc<AppState>` shared between both servers — auth middleware can access the same state
- Tower middleware available for both Axum and Tonic — natural place for auth interceptors
- `ServiceName` type (`types.rs`) already validates service names — auth scoping can reuse this

### Integration Points
- `main.rs:44-57` — gRPC server builder needs `.tls_config()` added for rustls
- `main.rs:64-74` — HTTP listener needs TLS acceptor wrapping `TcpListener`
- `http/submit.rs:27` — submit_task handler needs auth extraction before processing
- `grpc/poll.rs:31` — poll_tasks needs node token validation before streaming
- `grpc/submit.rs` (via TaskService) — needs mTLS client identity extraction
- New routes needed: `/v1/admin/api-keys` for key CRUD

</code_context>

<deferred>
## Deferred Ideas

- Admin API authentication — Phase 2 creates the endpoints; securing them (admin tokens or separate auth) can be addressed in Phase 3 or as an insertion
- API key rotation without downtime — listed as v2 requirement (EAUTH-02)
- Node authentication via mTLS certificates instead of pre-shared tokens — v2 requirement (EAUTH-01)
- Redis TLS (rediss://) — may add in Phase 5 with infrastructure hardening
- Single-port co-hosting with content-type routing — deferred from Phase 1, still deferred

</deferred>

---

*Phase: 02-authentication-and-tls*
*Context gathered: 2026-03-21*
