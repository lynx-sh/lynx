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
    /// Manage the Lynx background daemon
    Daemon(crate::commands::daemon::DaemonArgs),
    /// Switch or show context (interactive, agent, minimal)
    Context(crate::commands::context::ContextArgs),
    /// Diagnose issues with your Lynx setup
    Doctor(crate::commands::doctor::DoctorArgs),
    /// Benchmark startup time per component
    Benchmark(crate::commands::benchmark::BenchmarkArgs),
    /// Rollback config to a previous snapshot
    Rollback(crate::commands::rollback::RollbackArgs),
    /// Sync config via git
    Sync(crate::commands::sync::SyncArgs),
    /// Show, edit, or modify configuration
    Config(crate::commands::config::ConfigArgs),
    /// Run config schema migrations
    Migrate(crate::commands::migrate::MigrateArgs),
    /// Check for and install lx updates
    Update(crate::commands::update::UpdateArgs),
    /// Remove Lynx from this system
    Uninstall(crate::commands::uninstall::UninstallArgs),
}
