use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "lx",
    about = "Lynx — the shell framework that doesn't suck",
    version,
    propagate_version = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Initialize Lynx in a new shell session
    Init(crate::commands::init::InitArgs),
    /// Set up Lynx files in LYNX_DIR and optionally patch .zshrc
    Setup(crate::commands::setup::SetupArgs),
    /// Emit or subscribe to events
    Event(crate::commands::event::EventArgs),
    /// Manage plugins
    Plugin(crate::commands::plugin::PluginArgs),
    /// Manage themes
    Theme(crate::commands::theme::ThemeArgs),
    /// Manage scheduled cron tasks
    Cron(crate::commands::cron::CronArgs),
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
    /// Show real-world usage examples and quickstart guide
    Examples(crate::commands::examples::ExamplesArgs),
    /// Render PROMPT/RPROMPT for eval by shell precmd hook
    Prompt(crate::commands::prompt::PromptArgs),
    /// Emit zsh that populates _lynx_git_state (standalone / debugging)
    #[command(name = "git-state")]
    GitState(crate::commands::git::GitStateArgs),
    /// Emit zsh that populates _lynx_kubectl_state (standalone / debugging)
    #[command(name = "kubectl-state")]
    KubectlState(crate::commands::kubectl_state::KubectlStateArgs),
    /// Refresh all enabled plugin state caches in one eval (called by precmd hook)
    #[command(name = "refresh-state")]
    RefreshState(crate::commands::refresh_state::RefreshStateArgs),
    /// View and manage the Lynx diagnostic log
    Diag(crate::commands::diag::DiagArgs),
    /// Manage shell startup intros — ASCII art, system info, welcome messages
    Intro(crate::commands::intro::IntroArgs),
    /// Manage package registry taps (sources)
    Tap(crate::commands::tap::TapArgs),
    /// Install packages from the registry (tools, plugins, themes)
    Install(crate::commands::install::InstallPkgArgs),
    /// Remove a package's Lynx integration (preserves system binaries)
    Remove(crate::commands::install::UninstallPkgArgs),
    /// Browse available packages by category
    Browse(crate::commands::browse::BrowseArgs),
    /// Audit enabled plugins — show exports, hooks, binary deps, checksums
    Audit(crate::commands::audit::AuditArgs),
    /// Open the Lynx Dashboard — full management web UI
    Dashboard(crate::commands::dashboard::DashboardArgs),
    /// View and manage workflow jobs
    Jobs(crate::commands::jobs::JobsArgs),
    /// Execute a workflow
    Run(crate::commands::run::RunArgs),
    /// Interactive first-run setup wizard — pick theme, plugins, shell integration
    Onboard(crate::commands::onboard::OnboardArgs),
}
