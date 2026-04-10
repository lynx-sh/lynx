use anyhow::Result;
use clap::{Args, Subcommand};

#[derive(Args)]
pub struct PluginArgs {
    #[command(subcommand)]
    pub command: PluginCommand,
}

#[derive(Subcommand)]
pub enum PluginCommand {
    /// Install a plugin
    Add { name: String },
    /// Remove a plugin
    Remove { name: String },
    /// List installed plugins and their status
    List,
    /// Reinstall a plugin
    Reinstall { name: String },
    /// Scaffold a new plugin
    New { name: String },
}

pub async fn run(_args: PluginArgs) -> Result<()> {
    todo!("plugin commands")
}
