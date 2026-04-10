use anyhow::Result;
use clap::{Args, Subcommand};

#[derive(Args)]
pub struct SyncArgs {
    #[command(subcommand)]
    pub command: SyncCommand,
}

#[derive(Subcommand)]
pub enum SyncCommand {
    /// Initialize git-backed config sync
    Init { remote: String },
    /// Push config to remote
    Push,
    /// Pull config from remote
    Pull,
    /// Show sync status
    Status,
}

pub async fn run(_args: SyncArgs) -> Result<()> {
    todo!("sync commands")
}
