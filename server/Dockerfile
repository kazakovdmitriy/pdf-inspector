# Multi-stage build for the pdf-inspector HTTP API.
#
#   docker build -t pdf-inspector-api ./server
#   docker run -p 3000:3000 pdf-inspector-api
#
# The build context is the repo root (../), because the server crate depends
# on the library crate via a relative path. Build from the repository root:
#
#   docker build -f server/Dockerfile -t pdf-inspector-api .

# ---- Stage 1: build the release binary -------------------------------------
FROM rust:1-bookworm AS builder

# Pre-create the project layout so dependencies cache independently of source.
WORKDIR /build

# Copy manifests first to cache dependency compilation.
COPY Cargo.toml ./
COPY server/Cargo.toml ./server/Cargo.toml

# Create a stub so cargo can resolve the workspace before copying sources.
RUN mkdir -p src && echo "fn main() {}" > src/main.rs

# Build dependencies only. This layer is cached until a manifest changes.
RUN cargo build --release --manifest-path server/Cargo.toml || true

# Now copy the real source (library + server) and build the actual binary.
COPY src ./src
COPY external ./external
COPY server/src ./server/src

# Touching forces a rebuild of the binary now that real sources are in place.
RUN touch server/src/main.rs
RUN cargo build --release --manifest-path server/Cargo.toml

# ---- Stage 2: minimal runtime image ----------------------------------------
FROM debian:bookworm-slim AS runtime

# ca-certificates: any TLS for outbound calls if added later.
# curl: used by HEALTHCHECK. libssl via openssl is not required — the binary
# is pure Rust and statically links its TLS stack.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

# Non-root user for the running process.
RUN useradd --create-home --uid 10001 api
WORKDIR /home/api

# Copy the compiled binary out of the builder stage.
COPY --from=builder /build/target/release/pdf-inspector-api /usr/local/bin/pdf-inspector-api

USER api

EXPOSE 3000

# Env defaults; override at `docker run -e ...`.
ENV PORT=3000 \
    RUST_LOG=info \
    MAX_BODY_MB=100 \
    MAX_CONCURRENT=0 \
    QUEUE_TIMEOUT_SECS=60

HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    CMD curl -sf http://127.0.0.1:${PORT}/health || exit 1

ENTRYPOINT ["pdf-inspector-api"]
