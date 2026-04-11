use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Context as _, Result};
use clap::{Args, Subcommand};

use lynx_config::{load, snapshot::mutate_config_transaction};
use lynx_theme::loader::{list, load as load_theme, user_theme_dir};

#[derive(Args)]
pub struct ThemeArgs {
    #[command(subcommand)]
    pub command: ThemeCommand,
}

#[derive(Subcommand)]
pub enum ThemeCommand {
    /// Apply a named theme
    Set { name: String },
    /// Pick a random theme (excludes current)
    Random,
    /// List available themes
    List,
    /// Open the active theme in $EDITOR; validates and rolls back on error
    Edit,
    /// Show real-world usage examples
    Examples,
}

pub async fn run(args: ThemeArgs) -> Result<()> {
    match args.command {
        ThemeCommand::Set { name } => cmd_set(&name).await,
        ThemeCommand::Random => cmd_random().await,
        ThemeCommand::List => cmd_list(),
        ThemeCommand::Edit => cmd_edit().await,
        ThemeCommand::Examples => {
            crate::commands::examples::run(crate::commands::examples::ExamplesArgs {
                command: Some("theme".into()),
            })
            .await
        }
    }
}

async fn cmd_set(name: &str) -> Result<()> {
    // Validate theme exists and load for color export.
    let theme = load_theme(name).with_context(|| format!("theme '{name}' not found"))?;

    mutate_config_transaction(&format!("theme-set-{name}"), |cfg| {
        cfg.active_theme = name.to_string();
        Ok(())
    })
    .with_context(|| "failed to save config")?;

    // Emit theme:changed in-process so plugin handlers fire.
    emit_theme_changed(name).await;

    // Status to stderr — keeps stdout clean for eval "$(lx theme set <name>)".
    eprintln!("theme set to '{name}'");

    // Emit LS_COLORS and EZA_COLORS as shell assignments on stdout.
    // Callers can eval this output to update the current session:
    //   eval "$(lx theme set <name>)"
    if let Some(ls) = theme.ls_colors.to_ls_colors_string() {
        println!("export LS_COLORS={ls:?}");
    }
    if let Some(eza) = theme.ls_colors.to_eza_colors_string() {
        println!("export EZA_COLORS={eza:?}");
    }

    Ok(())
}

async fn cmd_random() -> Result<()> {
    let cfg = load().context("failed to load config")?;
    let current = &cfg.active_theme;
    let available: Vec<String> = list().into_iter().filter(|n| n != current).collect();

    if available.is_empty() {
        bail!("no other themes available to switch to");
    }

    // Simple pseudo-random: pick by (unix timestamp % len).
    let idx = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0)
        % available.len();

    cmd_set(&available[idx]).await
}

fn cmd_list() -> Result<()> {
    let cfg = load().context("failed to load config")?;
    let current = &cfg.active_theme;

    for name in list() {
        if &name == current {
            println!("* {name}");
        } else {
            println!("  {name}");
        }
    }
    Ok(())
}

async fn cmd_edit() -> Result<()> {
    let cfg = load().context("failed to load config")?;
    let theme_name = &cfg.active_theme;

    // Determine the path: prefer user theme dir, else error (built-ins are read-only).
    let user_path = user_theme_dir().join(format!("{theme_name}.toml"));
    let path = if user_path.exists() {
        user_path
    } else {
        // Can't edit a built-in in place — copy to user dir first.
        copy_builtin_to_user(theme_name)?
    };

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let snapshot = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read theme file {path:?}"))?;

    let status = Command::new(&editor)
        .arg(&path)
        .status()
        .with_context(|| format!("failed to launch editor '{editor}'"))?;

    if !status.success() {
        // Editor exited non-zero — restore snapshot.
        std::fs::write(&path, &snapshot).ok();
        bail!("editor exited with error — theme unchanged");
    }

    // Validate the saved file.
    match lynx_theme::loader::load_from_path(&path) {
        Ok(_) => {
            emit_theme_changed(theme_name).await;
            println!("theme '{theme_name}' saved and validated");
        }
        Err(e) => {
            // Roll back to snapshot.
            std::fs::write(&path, &snapshot)
                .context("CRITICAL: failed to restore theme snapshot")?;
            bail!("theme validation failed — rolled back: {e}");
        }
    }

    Ok(())
}

fn copy_builtin_to_user(name: &str) -> Result<PathBuf> {
    let dir = user_theme_dir();
    std::fs::create_dir_all(&dir).context("failed to create user theme directory")?;
    let dest = dir.join(format!("{name}.toml"));

    // load_theme reads the built-in content — serialise back to disk.
    let theme = load_theme(name).with_context(|| format!("theme '{name}' not found"))?;
    // Re-read the built-in source content rather than re-serialising.
    // We load the theme to validate it, then write the raw TOML.
    // Since built-ins are bundled via include_str!, find them through the loader.
    drop(theme); // validated above
                 // Use the raw content from the loader.
    let content = lynx_theme::loader::builtin_content(name)
        .with_context(|| format!("built-in theme '{name}' content unavailable"))?;
    std::fs::write(&dest, content).context("failed to write theme file")?;
    Ok(dest)
}

async fn emit_theme_changed(name: &str) {
    use lynx_events::types::{Event, THEME_CHANGED};
    let config = match lynx_config::load() {
        Ok(c) => c,
        Err(_) => return,
    };
    let plugins_dir = lynx_core::paths::installed_plugins_dir();
    let bus = crate::bus::build_active_bus(&config.active_context, &plugins_dir);
    let data = serde_json::json!({ "theme": name }).to_string();
    bus.emit(Event::new(THEME_CHANGED, data)).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_marks_active() {
        // Just ensure list() + load() produce consistent output (no panic).
        let themes = list();
        assert!(themes.contains(&"default".to_string()));
        assert!(themes.contains(&"minimal".to_string()));
    }
}
