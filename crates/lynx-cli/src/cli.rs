use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "lx",
    about = "Lynx — the shell framework that doesn't suck",
    version,
    propagate_version = true,
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Initialize Lynx in a new shell session
    Init(crate::commands::init::InitArgs),
    /// Emit or subscribe to events
    Event(crate::commands::event::EventArgs),
    /// Manage plugins
    Plugin(crate::commands::plugin::PluginArgs),
    /// Manage themes
    Theme(crate::commands::theme::ThemeArgs),
    /// Manage task scheduler
    Task(crate::commands::task::TaskArgs),
    /// Switch context (interactive, agent, minimal)
    Context { name: String },
    /// Diagnose issues with your Lynx setup
    Doctor,
    /// Benchmark startup time per component
    Benchmark,
    /// Rollback to last known good config
    Rollback,
    /// Sync config via git
    Sync(crate::commands::sync::SyncArgs),
}
