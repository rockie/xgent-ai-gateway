# Project Retrospective

*A living document updated after each milestone. Lessons feed forward into future planning.*

## Milestone: v1.0 — MVP

**Shipped:** 2026-03-22
**Phases:** 7 | **Plans:** 20 | **Tasks:** 41

### What Was Built
- Complete pull-model task gateway with gRPC + HTTPS dual protocol
- Full auth stack: API key, mTLS, node tokens with TLS termination
- Service registry with per-service queues and node health tracking
- Task reliability: background reaper + callback delivery with exponential backoff
- Production packaging: Prometheus metrics, structured JSON logging, static musl binary, Docker image
- gRPC auth hardening and integration fixes closing all audit gaps
- Sample echo service for end-to-end testing

### What Worked
- Phase-by-phase layering (queue → auth → registry → reliability → observability) kept each phase coherent and testable
- Redis Streams with consumer groups provided both reliable delivery and timeout detection (XPENDING IDLE) in one primitive
- Tower middleware sharing between Axum and Tonic eliminated duplicate auth logic
- Milestone audit after Phase 5 caught gRPC auth gaps early — Phases 6-7 closed them systematically
- ~5 min average per plan execution — tight feedback loops

### What Was Inefficient
- ROADMAP.md progress table wasn't kept in sync during execution (phases 2-7 show "Not started" despite completion)
- Phase 5 initially had 3 plans but needed a 4th (05-04) for integration test fixes — planning missed test compilation impact
- Reaper retry/DLQ scope wasn't decided until mid-Phase 4 — earlier descoping would have saved planning time

### Patterns Established
- Tower Service wrapper pattern for gRPC per-RPC auth layers
- Per-test Redis DB isolation via atomic counter for parallel integration tests
- Manual hyper-util TLS accept loop for per-connection keepalive control
- Config-based mTLS identity mapping (fingerprint → services) in gateway.toml
- `pub` test-friendly functions alongside private infinite-loop wrappers (reap_timed_out_tasks vs run_reaper)

### Key Lessons
1. Plan for integration test compilation cost — adding new AppState fields breaks all existing tests
2. Descope early and explicitly — "clients resubmit" is a valid v1 strategy that keeps the gateway simple
3. Milestone audits are worth the investment — the v1.0 audit found real gaps that would have shipped broken
4. Redis Streams > BLMOVE for this use case — consumer groups give reliable delivery without custom bookkeeping

### Cost Observations
- Total execution: ~103 minutes across 20 plans
- All work completed in 2 calendar days
- 8,429 LOC Rust shipped

---

## Cross-Milestone Trends

| Metric | v1.0 |
|--------|------|
| Phases | 7 |
| Plans | 20 |
| Tasks | 41 |
| Avg plan duration | ~5 min |
| LOC shipped | 8,429 |
| Calendar days | 2 |
