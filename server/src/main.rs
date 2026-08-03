//! pdf-inspector HTTP API server.
//!
//! Wraps the `pdf_inspector` library in a small REST service so it can be
//! deployed standalone and called from any language. See `server/README.md`.

mod dto;
mod error;
mod handlers;
mod state;

use std::net::SocketAddr;

use axum::http::Method;
use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

use state::{AppState, Config};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize structured logging. RUST_LOG controls verbosity
    // (e.g. `info,pdf_inspector_api=debug`).
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")))
        .init();

    let config = Config::from_env();
    let addr: SocketAddr = ([0, 0, 0, 0], config.port).into();
    let state = AppState::new(config);

    info!(
        port = state.config.port,
        max_body_mb = state.config.max_body_bytes / (1024 * 1024),
        max_concurrent = state.config.max_concurrent.get(),
        queue_timeout_secs = state.config.queue_timeout.as_secs(),
        "starting pdf-inspector-api"
    );

    let app = build_router(state.clone());

    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("server stopped");
    Ok(())
}

fn build_router(state: AppState) -> Router {
    let body_limit = RequestBodyLimitLayer::new(state.config.max_body_bytes);

    Router::new()
        .route("/convert", post(handlers::convert))
        .route("/detect", post(handlers::detect))
        .route("/health", get(handlers::health))
        .route("/", get(handlers::health))
        .layer(body_limit)
        .layer(CorsLayer::new()
            .allow_methods([Method::GET, Method::POST])
            .allow_origin(tower_http::cors::Any))
        // TraceLayer logs request method/path/status and latency.
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("installed ctrl-c handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("received SIGINT, shutting down"),
        _ = terminate => info!("received SIGTERM, shutting down"),
    }
}
