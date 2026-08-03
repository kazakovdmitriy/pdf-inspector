//! Application configuration and shared state.
//!
//! All knobs are configurable through environment variables so the same image
//! can be tuned without recompiling. See [`Config::from_env`].

use std::num::NonZero;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;

/// Server configuration parsed from the environment.
#[derive(Debug, Clone)]
pub struct Config {
    /// TCP port to listen on (default: 3000).
    pub port: u16,
    /// Maximum request body size in bytes (default: 100 MiB).
    pub max_body_bytes: usize,
    /// Max number of PDFs processed concurrently (default: number of CPUs).
    pub max_concurrent: NonZero<usize>,
    /// Timeout for acquiring a processing slot (default: 60s). When the server
    /// is at capacity, requests wait up to this long before returning 503.
    pub queue_timeout: Duration,
}

impl Config {
    /// Read configuration from environment variables, applying defaults.
    pub fn from_env() -> Self {
        let port = parse_env("PORT", 3000);
        let max_body_mb = parse_env("MAX_BODY_MB", 100usize);
        let cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        let max_concurrent = std::env::var("MAX_CONCURRENT")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .and_then(|n| n.max(1).try_into().ok())
            .unwrap_or_else(|| NonZero::new(cpus).expect("cpus >= 1"));
        let queue_timeout = Duration::from_secs(parse_env("QUEUE_TIMEOUT_SECS", 60u64));

        Self {
            port,
            max_body_bytes: max_body_mb.saturating_mul(1024 * 1024),
            max_concurrent,
            queue_timeout,
        }
    }
}

fn parse_env<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Shared state reachable from every handler.
#[derive(Clone)]
pub struct AppState {
    /// Bounded semaphore limiting concurrent CPU-heavy PDF processing.
    /// Acquired before parsing a PDF so the server stays responsive under load.
    pub processing_slots: Arc<Semaphore>,
    pub config: Arc<Config>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        Self {
            processing_slots: Arc::new(Semaphore::new(config.max_concurrent.get())),
            config: Arc::new(config),
        }
    }
}
