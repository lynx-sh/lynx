use anyhow::Result;
use clap::{Args, Subcommand};
use lynx_daemon::platform_backend;

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
    Status,
    /// Start the daemon service
    Start,
    /// Stop the daemon service
    Stop,
    /// Restart the daemon service
    Restart,
    /// Remove the daemon service
    Uninstall,
}

pub async fn run(args: DaemonArgs) -> Result<()> {
    let backend = platform_backend();

    match args.command {
        DaemonCommand::Install => {
            backend.install()?;
            println!("✓ lynx-daemon installed and started");
        }
        DaemonCommand::Status => {
            let status = backend.status()?;
            println!("lynx-daemon: {status}");
        }
        DaemonCommand::Start => {
            backend.start()?;
            println!("✓ lynx-daemon started");
        }
        DaemonCommand::Stop => {
            backend.stop()?;
            println!("✓ lynx-daemon stopped");
        }
        DaemonCommand::Restart => {
            backend.restart()?;
            println!("✓ lynx-daemon restarted");
        }
        DaemonCommand::Uninstall => {
            backend.uninstall()?;
            println!("✓ lynx-daemon removed");
        }
    }

    Ok(())
}
