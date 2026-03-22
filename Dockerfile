# Stage 1: Build static binary
FROM rust:latest AS builder

# Install musl target and protobuf compiler (needed by proto/build.rs)
RUN rustup target add x86_64-unknown-linux-musl
RUN apt-get update && apt-get install -y musl-tools protobuf-compiler && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy workspace manifests and lock file first for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY proto/Cargo.toml proto/
COPY gateway/Cargo.toml gateway/

# Create dummy source files to build and cache dependencies
RUN mkdir -p proto/src gateway/src gateway/src/bin && \
    echo "fn main() {}" > gateway/src/main.rs && \
    echo "" > gateway/src/lib.rs && \
    echo "" > proto/src/lib.rs && \
    echo "fn main() {}" > gateway/src/bin/agent.rs

# Copy proto build.rs and .proto files needed for codegen
COPY proto/build.rs proto/
COPY proto/src/gateway.proto proto/src/

# Build dependencies only (this layer is cached until Cargo.toml/Cargo.lock change)
RUN cargo build --release --target x86_64-unknown-linux-musl -p xgent-gateway 2>/dev/null || true

# Copy full source
COPY . .

# Touch source files to invalidate dummy builds
RUN touch gateway/src/main.rs gateway/src/lib.rs proto/src/lib.rs proto/build.rs

# Build the real binary
RUN cargo build --release --target x86_64-unknown-linux-musl

# Stage 2: Runtime image
FROM alpine:3.19

# Install CA certificates (for TLS connections to Redis, callbacks) and timezone data
RUN apk add --no-cache ca-certificates tzdata

# Copy the static binary
COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/xgent-gateway /usr/local/bin/xgent-gateway

# Copy default configuration
COPY gateway.toml /etc/xgent/gateway.toml

# Expose default ports (gRPC + HTTP)
EXPOSE 50051 8080

# Run with default config; override with volume mount or env vars
ENTRYPOINT ["xgent-gateway"]
CMD ["--config", "/etc/xgent/gateway.toml"]
