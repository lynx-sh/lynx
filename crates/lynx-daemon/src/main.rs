use anyhow::Result;
use lynx_core::runtime;
use lynx_task::{load_tasks, run_scheduler};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixListener;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("LYNX_LOG")
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    info!("lynx-daemon starting");

    // Write PID file so lx daemon status can check us.
    let pid_path = runtime::pid_file()?;
    std::fs::write(&pid_path, std::process::id().to_string())?;

    // Resolve paths.
    let tasks_path = tasks_toml_path();
    let log_dir = log_dir_path();

    // Load tasks.
    let tasks = load_tasks_safe(&tasks_path);
    let scheduler_handle = Arc::new(Mutex::new(Some(run_scheduler(tasks, log_dir.clone()))));

    // Open IPC socket.
    let socket_path = runtime::socket_path()?;
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }
    let listener = UnixListener::bind(&socket_path)?;
    info!("IPC socket open at {}", socket_path.display());

    // Spawn IPC accept loop.
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    tokio::spawn(async move {
                        handle_connection(stream).await;
                    });
                }
                Err(e) => {
                    error!("IPC accept error: {e}");
                }
            }
        }
    });

    // Signal handling.
    let mut sigterm = tokio::signal::unix::signal(
        tokio::signal::unix::SignalKind::terminate(),
    )?;
    let mut sighup = tokio::signal::unix::signal(
        tokio::signal::unix::SignalKind::hangup(),
    )?;

    loop {
        tokio::select! {
            _ = sigterm.recv() => {
                info!("SIGTERM received — shutting down");
                // Drop scheduler to abort all task loops.
                if let Ok(mut guard) = scheduler_handle.lock() {
                    *guard = None;
                }
                // Remove PID and socket files.
                let _ = std::fs::remove_file(&pid_path);
                let _ = std::fs::remove_file(&socket_path);
                break;
            }

            _ = sighup.recv() => {
                info!("SIGHUP received — reloading tasks");
                let new_tasks = load_tasks_safe(&tasks_path);
                if let Ok(mut guard) = scheduler_handle.lock() {
                    // Drop old scheduler (aborts all loops) and start fresh.
                    *guard = Some(run_scheduler(new_tasks, log_dir.clone()));
                }
                info!("tasks reloaded");
            }
        }
    }

    info!("lynx-daemon stopped");
    Ok(())
}

async fn handle_connection(stream: tokio::net::UnixStream) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break, // EOF
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                // Deserialize and handle IPC messages (event emit/subscribe).
                match serde_json::from_str::<serde_json::Value>(trimmed) {
                    Ok(msg) => {
                        info!(msg_type = ?msg.get("msg_type"), "IPC message received");
                        // Full event bus integration is handled by lynx-events;
                        // daemon acknowledges but doesn't re-emit back on this socket.
                    }
                    Err(e) => {
                        warn!("IPC bad JSON: {e} — line: {trimmed}");
                    }
                }
            }
            Err(e) => {
                error!("IPC read error: {e}");
                break;
            }
        }
    }
}

fn tasks_toml_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".config/lynx/tasks.toml")
}

fn log_dir_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".config/lynx/logs")
}

fn load_tasks_safe(path: &PathBuf) -> Vec<lynx_task::ValidatedTask> {
    if !path.exists() {
        info!("tasks.toml not found — no tasks scheduled");
        return Vec::new();
    }
    match load_tasks(path) {
        Ok(tasks) => {
            info!("loaded {} task(s) from {}", tasks.len(), path.display());
            tasks
        }
        Err(e) => {
            error!("failed to load tasks: {e}");
            Vec::new()
        }
    }
}
