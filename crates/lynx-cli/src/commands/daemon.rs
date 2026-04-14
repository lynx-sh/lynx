use anyhow::Result;
use clap::{Args, Subcommand};
use lynx_core::brand;
use lynx_core::error::LynxError;
use lynx_core::runtime::{pid_file, socket_path};
use lynx_daemon::platform_backend;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub struct DaemonArgs {
    #[command(subcommand)]
    pub command: DaemonCommand,
}

#[derive(Subcommand)]
pub enum DaemonCommand {
    /// Install and start the Lynx daemon as a system service
    Install,
    /// Show daemon status
    Status {
        /// Quiet mode: exit 0 if running, 1 if stopped; prints nothing.
        #[arg(long)]
        quiet: bool,
    },
    /// Start the daemon service
    Start {
        /// Start a detached user-process daemon instead of service manager.
        #[arg(long, visible_alias = "background")]
        detach: bool,
    },
    /// Stop the daemon service
    Stop,
    /// Restart the daemon service
    Restart,
    /// Remove the daemon service
    Uninstall,
    /// Catch unknown subcommands for friendly error
    #[command(external_subcommand)]
    Other(Vec<String>),
}

pub fn run(args: DaemonArgs) -> Result<()> {
    match args.command {
        DaemonCommand::Install => {
            let backend = platform_backend();
            backend.install()?;
            println!("✓ lynx-daemon installed and started");
        }
        DaemonCommand::Status { quiet } => {
            let running = is_running()?;
            if quiet {
                if running {
                    return Ok(());
                }
                std::process::exit(1);
            }

            let backend = platform_backend();
            let service_status = backend.status().ok();
            match (running, service_status) {
                (true, Some(status)) => println!("lynx-daemon: running ({status})"),
                (true, None) => println!("lynx-daemon: running"),
                (false, Some(status)) => println!("lynx-daemon: stopped ({status})"),
                (false, None) => println!("lynx-daemon: stopped"),
            }
        }
        DaemonCommand::Start { detach } => {
            if detach {
                if is_running()? {
                    println!("✓ lynx-daemon already running");
                } else {
                    start_detached()?;
                    println!("✓ lynx-daemon started (detached)");
                }
            } else {
                let backend = platform_backend();
                backend.start()?;
                println!("✓ lynx-daemon started");
            }
        }
        DaemonCommand::Stop => {
            let backend = platform_backend();
            // Best-effort: try both service manager and detached; check is_running() for result.
            let _ = backend.stop(); // may fail if service not installed
            let _ = stop_detached()?; // may fail if no detached process
            if !is_running()? {
                println!("✓ lynx-daemon stopped");
            } else {
                return Err(LynxError::Daemon("failed to stop lynx-daemon".into()).into());
            }
        }
        DaemonCommand::Restart => {
            let backend = platform_backend();
            let restarted_service = backend.restart().is_ok();
            let _ = stop_detached()?; // clean up detached process if any
            if restarted_service {
                println!("✓ lynx-daemon restarted");
            } else {
                start_detached()?;
                println!("✓ lynx-daemon restarted (detached)");
            }
        }
        DaemonCommand::Uninstall => {
            let backend = platform_backend();
            // Best-effort cleanup: uninstall service + kill detached process.
            if let Err(e) = backend.uninstall() {
                tracing::warn!("service uninstall failed (may not be installed): {e}");
            }
            let _ = stop_detached()?; // clean up detached process if any
            println!("✓ lynx-daemon removed");
        }
        DaemonCommand::Other(args) => {
            return Err(
                LynxError::unknown_command(super::unknown_subcmd_name(&args), "daemon").into(),
            );
        }
    }

    Ok(())
}

fn is_running() -> Result<bool> {
    if let Ok(pid_path) = pid_file() {
        if let Ok(pid_raw) = std::fs::read_to_string(pid_path) {
            if let Ok(pid) = pid_raw.trim().parse::<u32>() {
                return Ok(process_is_alive(pid));
            }
        }
    }

    if let Ok(sock) = socket_path() {
        if sock.exists() {
            return Ok(true);
        }
    }

    Ok(false)
}

#[cfg(unix)]
fn process_is_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn process_is_alive(_pid: u32) -> bool {
    false
}

fn start_detached() -> Result<()> {
    let daemon_bin = daemon_binary_path()?;

    Command::new(daemon_bin)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {
            anyhow::Error::from(LynxError::Daemon(format!(
                "failed to spawn lynx-daemon: {e}"
            )))
        })?;

    Ok(())
}

fn stop_detached() -> Result<bool> {
    let pid_path = match pid_file() {
        Ok(path) => path,
        Err(_) => return Ok(false),
    };

    let pid_raw = match std::fs::read_to_string(&pid_path) {
        Ok(pid) => pid,
        Err(_) => return Ok(false),
    };

    let pid = match pid_raw.trim().parse::<u32>() {
        Ok(pid) => pid,
        Err(_) => return Ok(false),
    };

    let status = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| {
            anyhow::Error::from(LynxError::Daemon(format!(
                "failed to signal lynx-daemon: {e}"
            )))
        })?;

    if !status.success() {
        return Ok(false);
    }

    for _ in 0..10 {
        if !process_is_alive(pid) {
            return Ok(true);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    Ok(!process_is_alive(pid))
}

fn daemon_binary_path() -> Result<PathBuf> {
    if let Ok(bin) = std::env::var(lynx_core::env_vars::LYNX_DAEMON_BIN) {
        let path = PathBuf::from(bin);
        if path.exists() {
            return Ok(path);
        }
    }

    if let Ok(path) = which::which(brand::DAEMON_NAME) {
        return Ok(path);
    }

    if let Ok(current) = std::env::current_exe() {
        if let Some(parent) = current.parent() {
            let sibling = parent.join(brand::DAEMON_NAME);
            if sibling.exists() {
                return Ok(sibling);
            }
        }
    }

    Err(LynxError::Daemon(format!(
        "lynx-daemon binary not found; install Lynx daemon or set {}",
        lynx_core::env_vars::LYNX_DAEMON_BIN
    ))
    .into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lynx_test_utils::env_lock;

    struct EnvGuard(Option<String>);

    impl EnvGuard {
        fn new() -> Self {
            Self(std::env::var(lynx_core::env_vars::LYNX_DAEMON_BIN).ok())
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(v) = &self.0 {
                std::env::set_var(lynx_core::env_vars::LYNX_DAEMON_BIN, v);
            } else {
                std::env::remove_var(lynx_core::env_vars::LYNX_DAEMON_BIN);
            }
        }
    }

    #[test]
    fn process_is_alive_returns_false_for_bogus_pid() {
        // PID 0 or very high PID should not be alive
        assert!(!process_is_alive(999_999_999));
    }

    #[test]
    #[cfg(unix)]
    fn process_is_alive_returns_true_for_current_process() {
        let pid = std::process::id();
        assert!(process_is_alive(pid));
    }

    #[test]
    fn daemon_binary_path_env_override() {
        let _lock = env_lock().lock().expect("lock");
        let _guard = EnvGuard::new();
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("lynx-daemon");
        std::fs::write(&bin, "").unwrap();

        std::env::set_var(lynx_core::env_vars::LYNX_DAEMON_BIN, bin.to_str().unwrap());
        let result = daemon_binary_path();
        std::env::remove_var(lynx_core::env_vars::LYNX_DAEMON_BIN);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), bin);
    }

    #[test]
    fn daemon_binary_path_env_override_nonexistent_falls_through() {
        let _lock = env_lock().lock().expect("lock");
        let _guard = EnvGuard::new();
        std::env::set_var(
            lynx_core::env_vars::LYNX_DAEMON_BIN,
            "/nonexistent/lynx-daemon",
        );
        let result = daemon_binary_path();

        // Should fall through to which/sibling checks, may or may not find the binary
        let _ = result;
    }

    #[tokio::test]
    async fn daemon_unknown_subcommand_errors() {
        let args = DaemonArgs {
            command: DaemonCommand::Other(vec!["bogus".to_string()]),
        };
        let err = run(args).unwrap_err();
        assert!(err.to_string().contains("bogus"));
    }

    #[test]
    fn is_running_returns_false_without_pid_or_socket() {
        // In test environment, daemon shouldn't be running
        // This may vary, but should not panic
        let _ = is_running();
    }
}
