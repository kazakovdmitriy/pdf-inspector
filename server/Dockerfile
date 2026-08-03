# Multi-stage build for the pdf-inspector HTTP API.
#
# Build from the repository root (the context must include both the library
# crate `src/` and the `server/` crate):
#
#   docker build -f server/Dockerfile -t pdf-inspector-api .
#
# Or via compose (see server/docker-compose.yml / stacks/pdf-api/compose.yaml).

# ---- Stage 1: build the release binary -------------------------------------
FROM rust:1-bookworm AS builder

WORKDIR /build

# Copy the entire repository (library + server). The library is a path
# dependency of the server crate, so both must be present.
COPY . .

# Build the release binary. Dependencies are compiled as part of this step;
# Docker layer caching still helps across rebuilds when source is unchanged.
RUN cargo build --release --manifest-path server/Cargo.toml

# ---- Stage 2: minimal runtime image ----------------------------------------
FROM debian:bookworm-slim AS runtime

# ca-certificates: for any outbound TLS added later.
# curl: used by HEALTHCHECK.
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

# Env defaults; override at `docker run -e ...` or in compose.
ENV PORT=3000 \
    RUST_LOG=info \
    MAX_BODY_MB=100 \
    MAX_CONCURRENT=0 \
    QUEUE_TIMEOUT_SECS=60

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -sf http://127.0.0.1:${PORT}/health || exit 1

ENTRYPOINT ["pdf-inspector-api"]
