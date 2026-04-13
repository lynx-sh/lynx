pub mod alias;
pub mod audit;
pub mod benchmark;
pub mod browse;
pub mod config;
pub mod context;
pub mod cron;
pub mod daemon;
pub mod dashboard;
pub mod diag;
pub mod doctor;
pub mod event;
pub mod examples;
pub mod git;
pub mod help;
pub mod init;
pub mod install;
pub mod intro;
pub mod jobs;
pub mod kubectl_state;
pub mod migrate;
pub mod nerd_font;
pub mod onboard;
pub mod path;
pub mod plugin;
pub mod prompt;
pub mod refresh_state;
pub mod rollback;
pub mod run;
pub mod setup;
pub mod sync;
pub mod tap;
pub mod theme;
mod theme_convert;
pub mod uninstall;
pub mod update;

use crate::cli::{Cli, Command};
use anyhow::Result;
use lynx_core::error::LynxError;

/// Extract the subcommand name from an external-subcommand args vec.
/// Returns the first element as `&str`, or `""` if the vec is empty.
pub(crate) fn unknown_subcmd_name(args: &[String]) -> &str {
    args.first().map(|s| s.as_str()).unwrap_or("")
}

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

/// Open a file in the user's preferred editor (blocking until the editor exits).
///
/// Respects `$VISUAL` first, then `$EDITOR`, falling back to `code` (VS Code).
/// Passes `--wait` only for GUI editors (`code`, `cursor`) that need it.
pub(crate) fn open_in_editor(path: &std::path::Path) -> Result<()> {
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "code".to_string());

    let editor_bin = std::path::Path::new(&editor)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&editor)
        .to_string();

    let needs_wait = editor_bin == "code" || editor_bin == "cursor";

    let mut cmd = std::process::Command::new(&editor);
    if needs_wait {
        cmd.arg("--wait");
    }
    cmd.arg(path);

    let status = cmd.status().map_err(|_| {
        anyhow::Error::from(LynxError::Shell(
            format!(
                "No editor found — set VISUAL or EDITOR env var, or install VS Code (tried: {editor})"
            )
        ))
    })?;

    if !status.success() {
        return Err(LynxError::Shell(
            format!("Editor `{editor}` exited with an error — file may not have been saved"),
        )
        .into());
    }
    Ok(())
}

pub async fn dispatch(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Init(args) => init::run(args),
        Command::Setup(args) => setup::run(args),
        Command::Event(args) => event::run(args).await,
        Command::Plugin(args) => plugin::run(args).await,
        Command::Theme(args) => theme::run(args).await,
        Command::Cron(args) => cron::run(args).await,
        Command::Daemon(args) => daemon::run(args),
        Command::Context(args) => context::run(args).await,
        Command::Doctor(args) => doctor::run(args),
        Command::Benchmark(args) => benchmark::run(args),
        Command::Rollback(args) => rollback::run(args),
        Command::Sync(args) => sync::run(args),
        Command::Config(args) => config::run(args),
        Command::Migrate(args) => migrate::run(args),
        Command::Update(args) => update::run(args),
        Command::Uninstall(args) => uninstall::run(args),
        Command::Examples(args) => examples::run(args),
        Command::Prompt(args) => prompt::run(args).await,
        Command::GitState(args) => git::run(args),
        Command::KubectlState(args) => kubectl_state::run(args),
        Command::RefreshState(args) => refresh_state::run(args),
        Command::Diag(args) => diag::run(args),
        Command::Intro(args) => intro::run(args),
        Command::Tap(args) => tap::run(args),
        Command::Install(args) => install::run_install(args).await,
        Command::Remove(args) => install::run_uninstall(args),
        Command::Browse(args) => browse::run(args),
        Command::Audit(args) => audit::run(args),
        Command::Dashboard(args) => dashboard::run(args).await,
        Command::Jobs(args) => jobs::run(args),
        Command::Run(args) => run::run(args).await,
        Command::Onboard(args) => onboard::run(args),
        Command::Alias(args) => alias::run(args),
        Command::Path(args) => path::run(args),
    }
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

    // Note: open_in_editor is not unit-testable without mocking Command.
    // It spawns VS Code with --wait which blocks. Tested via integration tests.
}
