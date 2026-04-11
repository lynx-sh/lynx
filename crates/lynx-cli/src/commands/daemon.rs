use anyhow::{anyhow, Result};
use clap::{Args, Subcommand};
use lynx_core::runtime::{pid_file, socket_path};
use lynx_daemon::platform_backend;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Args)]
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
}

pub async fn run(args: DaemonArgs) -> Result<()> {
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
            let _ = backend.stop();
            let _ = stop_detached()?;
            if !is_running()? {
                println!("✓ lynx-daemon stopped");
            } else {
                return Err(anyhow!("failed to stop lynx-daemon"));
            }
        }
        DaemonCommand::Restart => {
            let backend = platform_backend();
            let restarted_service = backend.restart().is_ok();
            let _ = stop_detached()?;
            if restarted_service {
                println!("✓ lynx-daemon restarted");
            } else {
                start_detached()?;
                println!("✓ lynx-daemon restarted (detached)");
            }
        }
        DaemonCommand::Uninstall => {
            let backend = platform_backend();
            let _ = backend.uninstall();
            let _ = stop_detached()?;
            println!("✓ lynx-daemon removed");
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

fn process_is_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn start_detached() -> Result<()> {
    let daemon_bin = daemon_binary_path()?;

    Command::new(daemon_bin)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| anyhow!("failed to spawn lynx-daemon: {e}"))?;

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
        .map_err(|e| anyhow!("failed to signal lynx-daemon: {e}"))?;

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
    if let Ok(bin) = std::env::var("LYNX_DAEMON_BIN") {
        let path = PathBuf::from(bin);
        if path.exists() {
            return Ok(path);
        }
    }

    if let Ok(path) = which::which("lynx-daemon") {
        return Ok(path);
    }

    if let Ok(current) = std::env::current_exe() {
        if let Some(parent) = current.parent() {
            let sibling = parent.join("lynx-daemon");
            if sibling.exists() {
                return Ok(sibling);
            }
        }
    }

    Err(anyhow!(
        "lynx-daemon binary not found; install Lynx daemon or set LYNX_DAEMON_BIN"
    ))
}
