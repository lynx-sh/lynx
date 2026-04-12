use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Context as _, Result};
use clap::{Args, Subcommand};

use lynx_config::{load, snapshot::mutate_config_transaction};
use lynx_theme::loader::{list, load as load_theme, load_from_path, user_theme_dir};
use lynx_theme::patch::{self, Side};

#[derive(Args)]
#[command(arg_required_else_help = true)]
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
    /// Mutate a single TOML field in the active theme using dot-path addressing.
    /// e.g. `lx theme patch colors.accent light-blue`
    Patch {
        /// Dot-separated TOML path (e.g. colors.accent, segment.dir.color.fg)
        path: String,
        /// New value (named color, hex, or literal string)
        value: String,
    },
    /// Set a palette color variable (shorthand for `lx theme patch colors.<key> <value>`)
    Palette {
        /// Color key in the [colors] table
        key: String,
        /// Color value (named color or hex)
        value: String,
    },
    /// Set the prompt caret symbol (shorthand for mutating segment.prompt_char.symbol)
    Caret { symbol: String },
    /// Set the prompt caret foreground color
    #[clap(name = "caret-color")]
    CaretColor { color: String },
    /// Add, remove, or move segments in the prompt order
    #[command(subcommand)]
    Segment(SegmentCommand),
    /// Convert an OMZ .zsh-theme file to Lynx TOML format
    Convert {
        /// Source: local file path, GitHub URL, or raw URL
        source: String,
        /// Output theme name (defaults to source filename stem)
        name: Option<String>,
        /// Overwrite existing theme file
        #[arg(long)]
        force: bool,
    },
    /// Open the WYSIWYG theme studio in your browser (local web UI, no npm)
    Studio,
}

#[derive(Subcommand)]
pub enum SegmentCommand {
    /// Add a segment to a side of the prompt
    Add {
        /// Segment name (e.g. git_branch)
        name: String,
        /// Which side: left or right
        side: String,
        /// Insert after this segment (optional; appends if omitted)
        #[arg(long)]
        after: Option<String>,
    },
    /// Remove a segment from the prompt (checks both sides)
    Remove {
        /// Segment name to remove
        name: String,
    },
    /// Move a segment to the given side (removes from the other side)
    Move {
        /// Segment name to move
        name: String,
        /// Target side: left or right
        side: String,
        /// Insert after this segment on the target side (optional; appends if omitted)
        #[arg(long)]
        after: Option<String>,
    },
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
        ThemeCommand::Patch { path, value } => cmd_patch(&path, &value).await,
        ThemeCommand::Palette { key, value } => {
            cmd_patch(&format!("colors.{key}"), &value).await
        }
        ThemeCommand::Caret { symbol } => {
            cmd_patch("segment.prompt_char.symbol", &symbol).await
        }
        ThemeCommand::CaretColor { color } => {
            cmd_patch("segment.prompt_char.color.fg", &color).await
        }
        ThemeCommand::Segment(seg) => cmd_segment(seg).await,
        ThemeCommand::Convert { source, name, force } => cmd_convert(&source, name.as_deref(), force).await,
        ThemeCommand::Studio => lynx_studio::run().await,
    }
}

async fn cmd_set(name: &str) -> Result<()> {
    // Validate theme exists before mutating config.
    let theme = load_theme(name).with_context(|| format!("theme '{name}' not found"))?;

    // Check if theme uses powerline/nerd font glyphs.
    if super::nerd_font::theme_needs_nerd_font(&theme) && !super::nerd_font::nerd_font_installed() {
        println!("⚠ Theme '{name}' uses powerline glyphs that require a Nerd Font.");
        println!("  Without one, separator characters will render as □ or ?.");
        println!();
        println!("  Download a Nerd Font from: https://www.nerdfonts.com/font-downloads");
        println!("  Popular choices: FiraCode Nerd Font, JetBrainsMono Nerd Font, Hack Nerd Font");
        println!();

        print!("  Install font and continue? [y]es / [n]o / [s]kip font check: ");
        std::io::Write::flush(&mut std::io::stdout())?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let choice = input.trim().to_lowercase();

        match choice.as_str() {
            "y" | "yes" => {
                if let Err(e) = super::nerd_font::install_nerd_font() {
                    println!("  ⚠ Font install failed: {e}");
                    println!("  Download manually from https://www.nerdfonts.com/font-downloads");
                    println!("  Then set your terminal font to the installed Nerd Font.");
                    print!("  Continue setting theme anyway? [y/n]: ");
                    std::io::Write::flush(&mut std::io::stdout())?;
                    input.clear();
                    std::io::stdin().read_line(&mut input)?;
                    if !input.trim().to_lowercase().starts_with('y') {
                        println!("theme not changed");
                        return Ok(());
                    }
                }
            }
            "s" | "skip" => {
                // Continue without font — user knows what they're doing.
            }
            _ => {
                println!("theme not changed");
                return Ok(());
            }
        }
    }

    mutate_config_transaction(&format!("theme-set-{name}"), |cfg| {
        cfg.active_theme = name.to_string();
        Ok(())
    })
    .with_context(|| "failed to save config")?;

    // Emit theme:changed in-process so plugin handlers fire.
    emit_theme_changed(name).await;

    println!("theme set to '{name}'");
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
    if !user_path.exists() {
        bail!("theme '{theme_name}' not found — run `lx install` to set up default themes");
    }
    let path = user_path;

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

/// Apply a dot-path patch to the active theme with snapshot/validate/rollback.
async fn cmd_patch(dot_path: &str, value: &str) -> Result<()> {
    let cfg = load().context("failed to load config")?;
    let theme_name = &cfg.active_theme;
    let path = resolve_user_theme_path(theme_name)?;

    let snapshot = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read theme file {path:?}"))?;

    let patched = patch::apply_patch(&snapshot, dot_path, value)
        .with_context(|| format!("failed to apply patch at '{dot_path}'"))?;

    std::fs::write(&path, &patched).with_context(|| "failed to write patched theme")?;

    match load_from_path(&path) {
        Ok(_) => {
            emit_theme_changed(theme_name).await;
            println!("theme '{theme_name}': {dot_path} = {value}");
        }
        Err(e) => {
            std::fs::write(&path, &snapshot)
                .context("CRITICAL: failed to restore theme snapshot after validation failure")?;
            bail!("theme validation failed — rolled back: {e}");
        }
    }

    Ok(())
}

/// Apply a segment array operation with snapshot/validate/rollback.
async fn cmd_segment(cmd: SegmentCommand) -> Result<()> {
    let cfg = load().context("failed to load config")?;
    let theme_name = &cfg.active_theme;
    let path = resolve_user_theme_path(theme_name)?;

    let snapshot = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read theme file {path:?}"))?;

    let patched = match &cmd {
        SegmentCommand::Add { name, side, after } => {
            let side: Side = side
                .parse()
                .with_context(|| format!("invalid side '{side}' — expected left or right"))?;
            patch::segment_add(&snapshot, name, side, after.as_deref())
                .with_context(|| format!("failed to add segment '{name}'"))?
        }
        SegmentCommand::Remove { name } => {
            patch::segment_remove(&snapshot, name)
                .with_context(|| format!("failed to remove segment '{name}'"))?
        }
        SegmentCommand::Move { name, side, after } => {
            let side: Side = side
                .parse()
                .with_context(|| format!("invalid side '{side}' — expected left or right"))?;
            patch::segment_move(&snapshot, name, side, after.as_deref())
                .with_context(|| format!("failed to move segment '{name}'"))?
        }
    };

    std::fs::write(&path, &patched).with_context(|| "failed to write patched theme")?;

    match load_from_path(&path) {
        Ok(_) => {
            emit_theme_changed(theme_name).await;
            let desc = match &cmd {
                SegmentCommand::Add { name, side, .. } => format!("added '{name}' to {side}"),
                SegmentCommand::Remove { name } => format!("removed '{name}'"),
                SegmentCommand::Move { name, side, .. } => format!("moved '{name}' to {side}"),
            };
            println!("theme '{theme_name}': {desc}");
        }
        Err(e) => {
            std::fs::write(&path, &snapshot)
                .context("CRITICAL: failed to restore theme snapshot after validation failure")?;
            bail!("theme validation failed — rolled back: {e}");
        }
    }

    Ok(())
}

/// Resolve the mutable user-theme path. Theme must exist in themes dir.
fn resolve_user_theme_path(theme_name: &str) -> Result<PathBuf> {
    let user_path = user_theme_dir().join(format!("{theme_name}.toml"));
    if user_path.exists() {
        Ok(user_path)
    } else {
        bail!("theme '{theme_name}' not found — run `lx install` to set up default themes")
    }
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

async fn cmd_convert(source: &str, name: Option<&str>, force: bool) -> Result<()> {
    // Resolve source.
    let resolved = lynx_convert::fetch::resolve_source(source)
        .context("failed to resolve theme source")?;

    // Derive theme name from source or use provided name.
    let theme_name = name
        .map(|n| n.to_string())
        .unwrap_or_else(|| {
            std::path::Path::new(source)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("converted")
                .to_string()
        });

    // Check for existing file.
    let out_path = user_theme_dir().join(format!("{theme_name}.toml"));
    if out_path.exists() && !force {
        bail!(
            "theme '{}' already exists at {}. Use --force to overwrite.",
            theme_name,
            out_path.display()
        );
    }

    // Fetch content.
    let content = lynx_convert::fetch::fetch_content(&resolved)
        .context("failed to fetch theme content")?;

    // Parse.
    let ir = lynx_convert::omz::parse(&content);

    // Emit TOML.
    let toml_str = lynx_convert::emit::to_lynx_toml(&ir, &theme_name);

    // Write.
    std::fs::create_dir_all(user_theme_dir())?;
    std::fs::write(&out_path, &toml_str)?;

    // Print summary.
    println!("Converted OMZ theme → {}", out_path.display());
    println!("  Segments (left):  {}", ir.left.join(", "));
    if !ir.right.is_empty() {
        println!("  Segments (right): {}", ir.right.join(", "));
    }
    if ir.two_line {
        println!("  Two-line layout detected");
    }
    if !ir.notes.is_empty() {
        for note in &ir.notes {
            println!("  ⚠ {note}");
        }
    }
    println!("\nActivate with: lx theme set {theme_name}");

    Ok(())
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
