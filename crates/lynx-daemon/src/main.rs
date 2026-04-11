use anyhow::Result;
use lynx_core::runtime;
use lynx_events::{bridge::IpcMessage, logger, types::Event, EventBus};
use lynx_task::{load_tasks, run_scheduler};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixListener;
use tracing::{error, info, warn};

// DispatchState is cloned into each connection handler task so every task
// gets a handle to the shared bus and subscriber registry without locking
// across await points.
#[derive(Clone)]
struct DispatchState {
    bus: Arc<EventBus>,
    subscribers: Arc<Mutex<HashMap<String, Vec<String>>>>,
}

impl DispatchState {
    fn new() -> Self {
        Self {
            bus: Arc::new(EventBus::new()),
            subscribers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn subscribe(&self, event_name: &str, zsh_fn: &str) -> Result<()> {
        validate_event_name(event_name)?;
        validate_subscriber_name(zsh_fn)?;

        {
            let mut lock = self
                .subscribers
                .lock()
                .map_err(|_| anyhow::anyhow!("subscriber registry lock poisoned"))?;
            lock.entry(event_name.to_string())
                .or_default()
                .push(zsh_fn.to_string());
        }

        let subscriber_source = format!("daemon:subscriber:{zsh_fn}");
        self.bus.subscribe(event_name, move |event| {
            let source = subscriber_source.clone();
            async move {
                let _ = logger::write_entry(&event, &source);
            }
        });

        Ok(())
    }

    async fn dispatch(&self, event: Event) -> usize {
        let _ = logger::write_entry(&event, "daemon:emit");
        self.bus.dispatch(event).await
    }

    fn subscriber_count(&self, event_name: &str) -> usize {
        self.subscribers
            .lock()
            .ok()
            .and_then(|map| map.get(event_name).map(|subs| subs.len()))
            .unwrap_or(0)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var(lynx_core::env_vars::LYNX_LOG).unwrap_or_else(|_| "info".into()))
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

    let dispatch_state = DispatchState::new();

    // Spawn IPC accept loop.
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let state = dispatch_state.clone();
                    tokio::spawn(async move {
                        handle_connection(stream, state).await;
                    });
                }
                Err(e) => {
                    error!("IPC accept error: {e}");
                }
            }
        }
    });

    // Signal handling.
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let mut sighup = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())?;

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

async fn handle_connection(stream: tokio::net::UnixStream, state: DispatchState) {
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
                match serde_json::from_str::<IpcMessage>(trimmed) {
                    Ok(msg) => {
                        if let Err(e) = handle_message(msg, &state).await {
                            warn!("IPC message rejected: {e}");
                        }
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

async fn handle_message(msg: IpcMessage, state: &DispatchState) -> Result<()> {
    match msg {
        IpcMessage::Emit { name, data } => {
            let event = Event::new(name.clone(), data);
            let dispatched = state.dispatch(event).await;
            info!(event = %name, handlers = dispatched, "event dispatched");
        }
        IpcMessage::Subscribe { event_name, zsh_fn } => {
            state.subscribe(&event_name, &zsh_fn)?;
            let count = state.subscriber_count(&event_name);
            info!(event = %event_name, zsh_fn = %zsh_fn, subscribers = count, "subscriber registered");
        }
    }
    Ok(())
}

fn validate_event_name(event_name: &str) -> Result<()> {
    if event_name.trim().is_empty() {
        anyhow::bail!("event_name cannot be empty");
    }
    if !event_name.contains(':') {
        anyhow::bail!("event_name must include namespace prefix (example: shell:precmd)");
    }
    Ok(())
}

fn validate_subscriber_name(zsh_fn: &str) -> Result<()> {
    if zsh_fn.trim().is_empty() {
        anyhow::bail!("subscriber function name cannot be empty");
    }
    if zsh_fn.chars().any(|c| c.is_whitespace()) {
        anyhow::bail!("subscriber function name cannot contain whitespace");
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[tokio::test]
    async fn subscribe_then_emit_dispatches_and_logs() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::env::set_var("HOME", tmp.path());

        let state = DispatchState::new();
        handle_message(
            IpcMessage::Subscribe {
                event_name: "shell:chpwd".into(),
                zsh_fn: "_test_hook".into(),
            },
            &state,
        )
        .await
        .expect("subscribe");
        handle_message(
            IpcMessage::Emit {
                name: "shell:chpwd".into(),
                data: "/tmp/project".into(),
            },
            &state,
        )
        .await
        .expect("emit");

        let entries = logger::tail_log(10, Some("shell:")).expect("tail");
        assert!(entries.iter().any(|e| e.source == "daemon:emit"));
        assert!(entries
            .iter()
            .any(|e| e.source == "daemon:subscriber:_test_hook"));

        std::env::remove_var("HOME");
    }

    #[tokio::test]
    async fn invalid_subscribe_is_rejected() {
        let state = DispatchState::new();
        let err = handle_message(
            IpcMessage::Subscribe {
                event_name: "shell:precmd".into(),
                zsh_fn: "bad name".into(),
            },
            &state,
        )
        .await
        .expect_err("expected invalid subscriber to fail");

        assert!(err.to_string().contains("cannot contain whitespace"));
    }

    #[tokio::test]
    async fn ipc_connection_dispatches_emit_and_writes_event_log() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::env::set_var("HOME", tmp.path());

        let state = DispatchState::new();
        let (server_std, mut client_std) =
            std::os::unix::net::UnixStream::pair().expect("unix pair");
        server_std
            .set_nonblocking(true)
            .expect("set server nonblocking");
        client_std
            .set_nonblocking(false)
            .expect("set client blocking");
        let server = tokio::net::UnixStream::from_std(server_std).expect("tokio server");

        let handle = tokio::spawn(async move {
            handle_connection(server, state).await;
        });

        let sub = serde_json::to_string(&IpcMessage::Subscribe {
            event_name: "shell:precmd".into(),
            zsh_fn: "_precmd_hook".into(),
        })
        .expect("serialize subscribe");
        let emit = serde_json::to_string(&IpcMessage::Emit {
            name: "shell:precmd".into(),
            data: "payload".into(),
        })
        .expect("serialize emit");

        client_std
            .write_all(format!("{sub}\n{emit}\n").as_bytes())
            .expect("write lines");
        drop(client_std);
        handle.await.expect("connection task");

        let entries = logger::tail_log(10, Some("shell:precmd")).expect("tail");
        assert!(!entries.is_empty(), "expected shell:precmd entries");
        assert!(entries.iter().any(|e| e.source == "daemon:emit"));
        assert!(entries
            .iter()
            .any(|e| e.source == "daemon:subscriber:_precmd_hook"));

        std::env::remove_var("HOME");
    }
}
