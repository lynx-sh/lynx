//! Lynx Theme Studio — local WYSIWYG theme editor.
//!
//! Starts an axum HTTP server on a random localhost port, opens the system
//! browser, and blocks until the user clicks "Apply" (POST /theme/apply) or
//! sends SIGINT. All theme mutations go through `lynx-theme::patch` and use
//! the snapshot/validate/rollback pipeline (D-007).
//!
//! No npm. No build step. Frontend is a single HTML file embedded in the
//! binary via `include_str!` (D-022).

pub mod server;

use anyhow::{Context as _, Result};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::info;

/// Run the studio: bind a random port, open browser, serve until /apply or SIGINT.
pub async fn run() -> Result<()> {
    // Bind on random OS-assigned port.
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("failed to bind studio port")?;
    let addr: SocketAddr = listener.local_addr()?;
    let url = format!("http://127.0.0.1:{}", addr.port());

    let state = server::AppState::new()?;
    let router = server::build_router(state);

    info!("lynx-studio listening on {url}");
    eprintln!("Lynx Theme Studio → {url}");
    eprintln!("Press Ctrl-C or click 'Apply' in the browser to exit.");

    // Open system browser (non-blocking — best-effort, ignore error).
    open_browser(&url);

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("studio server error")?;

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
