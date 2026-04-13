use std::path::PathBuf;

use anyhow::{Context as _, Result};
use lynx_core::error::LynxError;

use super::open_in_vscode;
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
    /// Convert an OMZ .zsh-theme or Oh-My-Posh .omp.json theme to Lynx TOML format
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
    /// Smart dispatch: treat unknown subcommand as theme name for `set`
    #[command(external_subcommand)]
    Other(Vec<String>),
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
        ThemeCommand::List => cmd_list().await,
        ThemeCommand::Edit => cmd_edit().await,
        ThemeCommand::Examples => {
            crate::commands::examples::run(crate::commands::examples::ExamplesArgs {
                command: Some("theme".into()),
            })
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
        ThemeCommand::Convert { source, name, force } => super::theme_convert::run(&source, name.as_deref(), force),
        ThemeCommand::Studio => {
            eprintln!("Note: `lx theme studio` is deprecated. Use `lx dashboard` instead.");
            lynx_dashboard::run().await
        }
        ThemeCommand::Other(args) => {
            if args.len() == 1 {
                cmd_set(&args[0]).await
            } else {
                Err(LynxError::unknown_command(args.first().map(|s| s.as_str()).unwrap_or(""), "theme").into())
            }
        }
    }
}

async fn cmd_set(name: &str) -> Result<()> {
    // Validate theme exists before mutating config.
    let theme = load_theme(name).with_context(|| format!("theme '{name}' not found"))?;

    // Check if theme uses powerline/nerd font glyphs.
    if super::nerd_font::theme_needs_nerd_font(&theme)
        && !super::nerd_font::ensure_nerd_font_ready()? {
            println!("theme not changed");
            return Ok(());
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
        return Err(LynxError::Theme("no other themes available to switch to".into()).into());
    }

    // Simple pseudo-random: pick by (unix timestamp % len).
    let idx = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0)
        % available.len();

    cmd_set(&available[idx]).await
}

/// A theme entry for the interactive list.
struct ThemeListEntry {
    name: String,
    description: String,
    author: String,
    segments: Vec<String>,
    palette_keys: Vec<String>,
    is_current: bool,
}

impl lynx_tui::ListItem for ThemeListEntry {
    fn title(&self) -> &str {
        &self.name
    }

    fn subtitle(&self) -> String {
        if self.description.is_empty() {
            String::new()
        } else {
            self.description.clone()
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
        if !self.segments.is_empty() {
            lines.push(String::new());
            lines.push(format!("Segments: {}", self.segments.join(", ")));
        }
        if !self.palette_keys.is_empty() {
            lines.push(String::new());
            lines.push(format!("Palette: {}", self.palette_keys.join(", ")));
        }
        if self.is_current {
            lines.push(String::new());
            lines.push("(active theme)".to_string());
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

async fn cmd_list() -> Result<()> {
    let cfg = load().context("failed to load config")?;
    let current = &cfg.active_theme;

    let names = list();
    let entries: Vec<ThemeListEntry> = names
        .iter()
        .map(|name| {
            let (description, author, segments, palette_keys) =
                match load_theme(name) {
                    Ok(theme) => {
                        let desc = theme.meta.description.clone();
                        let auth = theme.meta.author.clone();
                        let segs: Vec<String> = theme.segments.top.order.iter()
                            .chain(theme.segments.left.order.iter())
                            .chain(theme.segments.right.order.iter())
                            .cloned()
                            .collect();
                        let pal: Vec<String> = {
                            let mut keys: Vec<String> = theme.colors.keys().cloned().collect();
                            keys.sort();
                            keys
                        };
                        (desc, auth, segs, pal)
                    }
                    Err(_) => (String::new(), String::new(), vec![], vec![]),
                };
            ThemeListEntry {
                name: name.clone(),
                description,
                author,
                segments,
                palette_keys,
                is_current: name == current,
            }
        })
        .collect();

    // Load TUI colors from active theme.
    let tui_colors = match load_theme(current) {
        Ok(theme) => lynx_tui::TuiColors::from_palette(&theme.colors),
        Err(_) => lynx_tui::TuiColors::default(),
    };

    if let Some(idx) = lynx_tui::show(&entries, "Themes", &tui_colors)? {
        let selected = &entries[idx].name;
        if selected != current {
            cmd_set(selected).await?;
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
        return Err(LynxError::NotFound { item_type: "Theme".into(), name: theme_name.to_string(), hint: "run `lx setup` to set up default themes".into() }.into());
    }
    let path = user_path;

    let snapshot = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read theme file {path:?}"))?;

    open_in_vscode(&path)?;

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
            return Err(LynxError::Theme(format!("theme validation failed — rolled back: {e}")).into());
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
            return Err(LynxError::Theme(format!("theme validation failed — rolled back: {e}")).into());
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
            return Err(LynxError::Theme(format!("theme validation failed — rolled back: {e}")).into());
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
        Err(LynxError::NotFound { item_type: "Theme".into(), name: theme_name.to_string(), hint: "run `lx setup` to set up default themes".into() }.into())
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
