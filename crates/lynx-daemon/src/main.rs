use anyhow::Result;
use lynx_core::runtime;
use lynx_task::{load_tasks, run_scheduler};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var(lynx_core::env_vars::LYNX_LOG).unwrap_or_else(|_| "info".into()))
        .init();

    info!("lynx-daemon starting");

    // Write PID file so lx daemon status can check us.
    let pid_path = runtime::pid_file()?;
    std::fs::write(&pid_path, std::process::id().to_string())?;

    let tasks_path = tasks_toml_path();
    let log_dir = log_dir_path();

    let tasks = load_tasks_safe(&tasks_path);
    let scheduler_handle = Arc::new(Mutex::new(Some(run_scheduler(tasks, log_dir.clone()))));

    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let mut sighup = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())?;

    loop {
        tokio::select! {
            _ = sigterm.recv() => {
                info!("SIGTERM received — shutting down");
                if let Ok(mut guard) = scheduler_handle.lock() {
                    *guard = None;
                }
                let _ = std::fs::remove_file(&pid_path);
                break;
            }

            _ = sighup.recv() => {
                info!("SIGHUP received — reloading tasks");
                let new_tasks = load_tasks_safe(&tasks_path);
                if let Ok(mut guard) = scheduler_handle.lock() {
                    *guard = Some(run_scheduler(new_tasks, log_dir.clone()));
                }
                info!("tasks reloaded");
            }
        }
    }

    info!("lynx-daemon stopped");
    Ok(())
}

fn tasks_toml_path() -> PathBuf {
    lynx_core::paths::tasks_file()
}

fn log_dir_path() -> PathBuf {
    lynx_core::paths::logs_dir()
}

fn load_tasks_safe(path: &Path) -> Vec<lynx_task::ValidatedTask> {
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
