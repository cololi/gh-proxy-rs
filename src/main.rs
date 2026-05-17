//! Binary entry point. Loads configuration, builds the shared HTTP client,
//! wires the axum router, and serves until SIGINT/SIGTERM.

use std::sync::Arc;
use std::time::Duration;

use hub_proxy::{config, proxy};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cfg = config::Config::from_env();
    tracing::info!(
        listen = %cfg.listen,
        size_limit = cfg.size_limit,
        "Hub-Proxy 正在启动"
    );

    // Normalize ":8080" → "0.0.0.0:8080" for socket binding.
    let addr_str = if cfg.listen.starts_with(':') {
        format!("0.0.0.0{}", cfg.listen)
    } else {
        cfg.listen.clone()
    };
    let listener = tokio::net::TcpListener::bind(&addr_str).await?;
    tracing::info!("已绑定 {addr_str}");

    let client = reqwest::Client::builder()
        .timeout(cfg.upstream_timeout)
        .connect_timeout(cfg.upstream_timeout)
        .pool_max_idle_per_host(20)
        .pool_idle_timeout(Duration::from_secs(90))
        .http2_adaptive_window(true)
        .redirect(reqwest::redirect::Policy::none())
        .no_gzip()
        .build()?;

    // `shutdown_timeout` is exposed so it can be wired into a hard deadline in
    // future; axum::serve's built-in graceful shutdown drains hyper connections
    // as soon as the signal future resolves.
    let _shutdown_timeout = cfg.shutdown_timeout;
    let state = Arc::new(proxy::AppState { cfg, client });
    let app = proxy::router(state);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("已退出");
    Ok(())
}

async fn shutdown_signal() {
    use tokio::signal::unix::{signal, SignalKind};
    let mut sigint = signal(SignalKind::interrupt()).expect("install SIGINT handler");
    let mut sigterm = signal(SignalKind::terminate()).expect("install SIGTERM handler");
    tokio::select! {
        _ = sigint.recv() => {}
        _ = sigterm.recv() => {}
    }
    tracing::info!("正在关闭服务器...");
}
