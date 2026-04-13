pub mod benchmark;
pub mod help;
pub mod dashboard;
pub mod intro;
pub mod diag;
pub mod config;
pub mod context;
pub mod daemon;
pub mod doctor;
pub mod git;
pub mod kubectl_state;
pub mod refresh_state;
pub mod event;
pub mod examples;
pub mod init;
pub mod jobs;
pub mod setup;
pub mod migrate;
pub mod plugin;
pub mod prompt;
pub mod rollback;
pub mod run;
pub mod sync;
pub mod audit;
pub mod browse;
pub mod install;
pub mod tap;
pub mod cron;
pub mod nerd_font;
pub mod theme;
mod theme_convert;
pub mod uninstall;
pub mod update;

use crate::cli::{Cli, Command};
use anyhow::{Result};
use lynx_core::error::LynxError;

/// Load TUI colors from the active theme. Falls back to defaults.
pub(crate) fn tui_colors() -> lynx_tui::TuiColors {
    let Ok(cfg) = lynx_config::load() else {
        return lynx_tui::TuiColors::default();
    };
    match lynx_theme::loader::load(&cfg.active_theme) {
        Ok(theme) => lynx_tui::TuiColors::from_palette(&theme.colors),
        Err(_) => lynx_tui::TuiColors::default(),
    }
}

/// Open a file in VS Code (blocking until the window/tab is closed).
/// Errors with a clear install message if `code` is not in PATH.
pub(crate) fn open_in_vscode(path: &std::path::Path) -> Result<()> {
    let status = std::process::Command::new("code")
        .arg("--wait")
        .arg(path)
        .status()
        .map_err(|_| anyhow::Error::from(lynx_core::error::LynxError::Shell(
            "VS Code is required to edit this file — install from https://code.visualstudio.com and ensure `code` is in PATH".into()
        )))?;

    if !status.success() {
        return Err(LynxError::Shell("VS Code exited with an error — file may not have been saved".into()).into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tui_colors_returns_default_without_config() {
        // In test environment, config may not exist — should fallback gracefully.
        let colors = tui_colors();
        // Just ensure it doesn't panic and returns something.
        let _ = colors;
    }

    // Note: open_in_vscode is not unit-testable without mocking Command.
    // It spawns VS Code with --wait which blocks. Tested via integration tests.
}

pub async fn dispatch(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Init(args) => init::run(args).await,
        Command::Setup(args) => setup::run(args).await,
        Command::Event(args) => event::run(args).await,
        Command::Plugin(args) => plugin::run(args).await,
        Command::Theme(args) => theme::run(args).await,
        Command::Cron(args) => cron::run(args).await,
        Command::Daemon(args) => daemon::run(args).await,
        Command::Context(args) => context::run(args).await,
        Command::Doctor(args) => doctor::run(args).await,
        Command::Benchmark(args) => benchmark::run(args).await,
        Command::Rollback(args) => rollback::run(args).await,
        Command::Sync(args) => sync::run(args).await,
        Command::Config(args) => config::run(args).await,
        Command::Migrate(args) => migrate::run(args).await,
        Command::Update(args) => update::run(args).await,
        Command::Uninstall(args) => uninstall::run(args).await,
        Command::Examples(args) => examples::run(args).await,
        Command::Prompt(args) => prompt::run(args).await,
        Command::GitState(args) => git::run(args).await,
        Command::KubectlState(args) => kubectl_state::run(args).await,
        Command::RefreshState(args) => refresh_state::run(args).await,
        Command::Diag(args) => diag::run(args).await,
        Command::Intro(args) => intro::run(args).await,
        Command::Tap(args) => tap::run(args).await,
        Command::Install(args) => install::run_install(args).await,
        Command::Remove(args) => install::run_uninstall(args).await,
        Command::Browse(args) => browse::run(args).await,
        Command::Audit(args) => audit::run(args).await,
        Command::Dashboard(args) => dashboard::run(args).await,
        Command::Jobs(args) => jobs::run(args).await,
        Command::Run(args) => run::run(args).await,
    }
}
