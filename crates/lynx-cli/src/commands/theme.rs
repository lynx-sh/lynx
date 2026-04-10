use anyhow::Result;
use clap::{Args, Subcommand};

#[derive(Args)]
pub struct ThemeArgs {
    #[command(subcommand)]
    pub command: ThemeCommand,
}

#[derive(Subcommand)]
pub enum ThemeCommand {
    /// Apply a theme
    Set { name: String },
    /// Pick a random theme
    Random,
    /// List available themes
    List,
    /// Open active theme in $EDITOR
    Edit,
}

pub async fn run(_args: ThemeArgs) -> Result<()> {
    todo!("theme commands")
}
