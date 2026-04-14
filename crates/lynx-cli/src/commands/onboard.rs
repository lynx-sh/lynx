//! `lx onboard` — interactive first-run setup wizard.
//!
//! Walks the user through:
//! 1. Welcome screen (Info)
//! 2. Theme picker (InteractiveList)
//! 3. Plugin picker (show_multi)
//! 4. Shell integration confirm (zshrc patch)
//! 5. Done screen
//!
//! All config changes are collected and written once at the end (D-007).
//! Falls back to plain terminal prompts when not interactive (non-TTY, agent, etc.).
//! Auto-launched by `lx setup` when stdout is a TTY and onboarding is not yet complete.

use anyhow::{Context, Result};
use clap::Args;
use lynx_config::{load as load_config, save as save_config};
use lynx_core::brand;
use lynx_tui::onboard::{OnboardResult, OnboardStep, OnboardStepKind};

use super::setup::patch_zshrc;

#[derive(Args)]
pub struct OnboardArgs {
    /// Force re-run even if onboarding was already completed.
    #[arg(long)]
    pub force: bool,
}

pub fn run(args: OnboardArgs) -> Result<()> {
    let mut cfg = load_config().context("failed to load config")?;

    if cfg.onboarding_complete && !args.force {
        println!("Lynx is already set up. Run `lx onboard --force` to redo the wizard.");
        return Ok(());
    }

    let colors = super::tui_colors();

    // ── Step 1: Welcome ──────────────────────────────────────────────────────
    let welcome_body = format!(
        "\
Welcome to {name}!\n\
\n\
{name} is a Rust-powered zsh framework that gives you a fast, themed shell\n\
with plugins, workflows, and full customisation — all in TOML.\n\
\n\
This wizard will help you:\n\
  • Pick a theme\n\
  • Choose which plugins to enable\n\
  • Wire Lynx into your shell (optional)\n\
\n\
You can re-run `lx onboard` any time to revisit these settings.",
        name = brand::NAME
    );

    let wizard_steps = vec![
        OnboardStep {
            title: format!("Welcome to {}", brand::NAME),
            body: welcome_body,
            kind: OnboardStepKind::Info,
        },
        OnboardStep {
            title: "Shell integration".into(),
            body: "\
Do you want Lynx to add the init line to your ~/.zshrc?\n\
\n\
This enables Lynx automatically in every new terminal session.\n\
You can also do this manually later with `lx setup --zshrc`."
                .into(),
            kind: OnboardStepKind::Confirm {
                prompt: "Patch ~/.zshrc?".into(),
                default: true,
            },
        },
        OnboardStep {
            title: "All set!".into(),
            body: "\
Setup complete! Here's what to do next:\n\
\n\
  • Restart your shell or run: source ~/.zshrc\n\
  • Browse themes:             lx theme list\n\
  • Enable more plugins:       lx plugin list\n\
  • View the dashboard:        lx dashboard\n\
  • Get help:                  lx help"
                .into(),
            kind: OnboardStepKind::Done,
        },
    ];

    // ── Run Info / Confirm steps ─────────────────────────────────────────────
    let results = lynx_tui::onboard::run_onboard_wizard(&wizard_steps, &colors)
        .context("onboard wizard failed")?;

    // Wizard was quit early — don't write config.
    if results.iter().any(|r| r == &OnboardResult::Quit) {
        println!("\nWizard cancelled. Run `lx onboard` to try again.");
        return Ok(());
    }

    // ── Step 2: Theme picker (between wizard steps 0 and 1) ─────────────────
    let selected_theme = pick_theme(&cfg.active_theme, &colors)?;
    if let Some(theme) = selected_theme {
        cfg.active_theme = theme;
    }

    // ── Step 3: Plugin picker ────────────────────────────────────────────────
    let selected_plugins = pick_plugins(&cfg.enabled_plugins, &colors)?;
    if !selected_plugins.is_empty() {
        cfg.enabled_plugins = selected_plugins;
    }

    // ── Apply shell integration if confirmed ─────────────────────────────────
    // Wizard step index 1 is the zshrc Confirm step.
    if let Some(OnboardResult::Confirmed(true)) = results.get(1) {
        let home = lynx_core::paths::home();
        if let Err(e) = patch_zshrc(&home) {
            // Non-fatal — warn and continue.
            eprintln!("  ⚠ Could not patch ~/.zshrc: {e}");
            eprintln!("    Add this line manually: {}", brand::ZSHRC_INIT_LINE);
        }
    }

    // ── Write config once ────────────────────────────────────────────────────
    cfg.onboarding_complete = true;
    save_config(&cfg).context("failed to write config after onboarding")?;

    Ok(())
}

// ── Theme picker ─────────────────────────────────────────────────────────────

/// Display available themes in an InteractiveList and return the selected name,
/// or None if cancelled / non-interactive.
fn pick_theme(current: &str, colors: &lynx_tui::TuiColors) -> Result<Option<String>> {
    let names = lynx_theme::loader::list();
    if names.is_empty() {
        return Ok(None);
    }

    let entries: Vec<ThemeEntry> = names
        .iter()
        .map(|name| {
            let (desc, auth) = match lynx_theme::loader::load(name) {
                Ok(t) => (t.meta.description, t.meta.author),
                Err(_) => (String::new(), String::new()),
            };
            ThemeEntry {
                name: name.clone(),
                description: desc,
                author: auth,
                is_current: name == current,
            }
        })
        .collect();

    println!("\n── Theme selection ──────────────────────────────────────────────");
    let selected = lynx_tui::show(&entries, "Pick a theme", colors)?;
    Ok(selected.map(|idx| entries[idx].name.clone()))
}

struct ThemeEntry {
    name: String,
    description: String,
    author: String,
    is_current: bool,
}

impl lynx_tui::ListItem for ThemeEntry {
    fn title(&self) -> &str {
        &self.name
    }
    fn subtitle(&self) -> String {
        if !self.description.is_empty() {
            self.description.clone()
        } else if !self.author.is_empty() {
            format!("by {}", self.author)
        } else {
            String::new()
        }
    }
    fn detail(&self) -> String {
        let mut lines = Vec::new();
        if !self.description.is_empty() {
            lines.push(self.description.clone());
        }
        if !self.author.is_empty() {
            lines.push(format!("Author: {}", self.author));
        }
        if self.is_current {
            lines.push("\n(currently active)".to_string());
        }
        lines.join("\n")
    }
    fn category(&self) -> Option<&str> {
        Some("theme")
    }
    fn is_active(&self) -> bool {
        self.is_current
    }
}

// ── Plugin picker ─────────────────────────────────────────────────────────────

/// Display available installed plugins in a multi-select list.
/// Returns the selected plugin names, or the current list if cancelled.
fn pick_plugins(current: &[String], colors: &lynx_tui::TuiColors) -> Result<Vec<String>> {
    let plugin_dir = lynx_core::paths::installed_plugins_dir();
    let entries = discover_plugins(&plugin_dir);

    if entries.is_empty() {
        return Ok(current.to_vec());
    }

    // Pre-select plugins that are already enabled.
    let preselected: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| current.contains(&e.name))
        .map(|(i, _)| i)
        .collect();

    println!("\n── Plugin selection ─────────────────────────────────────────────");
    let selected_indices = lynx_tui::show_multi(&entries, "Enable plugins", colors, &preselected)?;

    if selected_indices.is_empty() && preselected.is_empty() {
        // Cancelled with nothing to restore — keep current.
        return Ok(current.to_vec());
    }

    let selected_names: Vec<String> = selected_indices
        .iter()
        .map(|&i| entries[i].name.clone())
        .collect();

    Ok(selected_names)
}

/// Discover plugins by reading the installed plugins directory.
fn discover_plugins(plugin_dir: &std::path::Path) -> Vec<PluginEntry> {
    let Ok(entries) = std::fs::read_dir(plugin_dir) else {
        return vec![];
    };

    let mut plugins: Vec<PluginEntry> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let description = read_plugin_description(&e.path());
            PluginEntry { name, description }
        })
        .collect();

    plugins.sort_by(|a, b| a.name.cmp(&b.name));
    plugins
}

/// Best-effort: read description from plugin.toml.
fn read_plugin_description(plugin_path: &std::path::Path) -> String {
    let toml_path = plugin_path.join("plugin.toml");
    let Ok(content) = std::fs::read_to_string(toml_path) else {
        return String::new();
    };
    let Ok(manifest) = lynx_manifest::parse_and_validate(&content) else {
        return String::new();
    };
    manifest.plugin.description
}

struct PluginEntry {
    name: String,
    description: String,
}

impl lynx_tui::ListItem for PluginEntry {
    fn title(&self) -> &str {
        &self.name
    }
    fn subtitle(&self) -> String {
        self.description.clone()
    }
    fn detail(&self) -> String {
        if self.description.is_empty() {
            format!("Plugin: {}", self.name)
        } else {
            format!("Plugin: {}\n\n{}", self.name, self.description)
        }
    }
    fn category(&self) -> Option<&str> {
        Some("plugin")
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_plugins_missing_dir_returns_empty() {
        let result = discover_plugins(std::path::Path::new("/nonexistent/dir"));
        assert!(result.is_empty());
    }

    #[test]
    fn read_plugin_description_missing_file_returns_empty() {
        let desc = read_plugin_description(std::path::Path::new("/nonexistent"));
        assert!(desc.is_empty());
    }

    #[test]
    fn onboard_args_force_default_false() {
        use clap::Parser;
        #[derive(Parser)]
        struct W {
            #[command(flatten)]
            args: OnboardArgs,
        }
        let w = W::parse_from(["test"]);
        assert!(!w.args.force);
    }

    #[test]
    fn onboard_args_force_flag() {
        use clap::Parser;
        #[derive(Parser)]
        struct W {
            #[command(flatten)]
            args: OnboardArgs,
        }
        let w = W::parse_from(["test", "--force"]);
        assert!(w.args.force);
    }
}
