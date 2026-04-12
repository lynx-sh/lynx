//! Lynx Dashboard — local web UI for full shell framework management.
//!
//! Starts an axum HTTP server on a random localhost port, opens the system
//! browser, and blocks until the user sends SIGINT. All mutations go through
//! library crate APIs using the snapshot/validate/apply pipeline (D-007).
//!
//! No npm. No build step. Frontend is modular vanilla HTML/CSS/JS embedded
//! via `include_str!` (D-035).

pub mod api;
pub mod frontend;
pub mod server;

use anyhow::{Context as _, Result};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::info;

/// Start the dashboard server on a random port, open the browser, block until Ctrl-C.
pub async fn run() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("failed to bind dashboard port")?;
    let addr: SocketAddr = listener.local_addr()?;
    let url = format!("http://127.0.0.1:{}", addr.port());

    let state = server::AppState::new();
    let router = server::build_router(state);

    info!("lynx-dashboard listening on {url}");
    eprintln!("Lynx Dashboard → {url}");
    eprintln!("Press Ctrl-C to stop.");

    open_browser(&url);

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("dashboard server error")?;

    Ok(())
}

fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/c", "start", url])
        .spawn();
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl-C handler");
}
