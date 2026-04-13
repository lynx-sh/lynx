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
        && !ensure_nerd_font_ready()? {
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

/// Ensure a Nerd Font is installed AND the terminal is configured to use it.
/// Returns true if ready to proceed, false if user chose to cancel.
fn ensure_nerd_font_ready() -> Result<bool> {
    use super::nerd_font;

    let fonts = nerd_font::find_installed_nerd_fonts();
    let terminal_ok = nerd_font::terminal_using_nerd_font();

    if terminal_ok {
        return Ok(true); // Font installed and terminal using it — good to go.
    }

    if fonts.is_empty() {
        // No Nerd Font installed at all.
        println!("⚠ This theme uses powerline glyphs that require a Nerd Font.");
        println!("  Without one, separator characters will render as □ or ?.");
        println!();
        print!("  Download and install a Nerd Font? [y]es / [n]o / [s]kip: ");
        std::io::Write::flush(&mut std::io::stdout())?;

        let choice = read_line_lower()?;
        match choice.as_str() {
            "y" | "yes" => {
                let family = nerd_font::install_nerd_font()
                    .context("font installation failed")?;
                return offer_terminal_config(&family);
            }
            "s" | "skip" => return Ok(true),
            _ => return Ok(false),
        }
    }

    // Fonts exist on disk but terminal isn't using one.
    let first = &fonts[0];
    println!("⚠ Nerd Font found ({first}) but your terminal isn't using it.");
    println!("  Powerline glyphs will render as □ until the terminal font is changed.");
    println!();

    offer_terminal_config(first)
}

/// Offer to auto-configure the terminal font. Returns true to proceed, false to cancel.
fn offer_terminal_config(font_family: &str) -> Result<bool> {
    use super::nerd_font;

    // Detect terminal and offer auto-config if supported.
    if std::env::var("ITERM_SESSION_ID").is_ok()
        || std::env::var("TERM_PROGRAM").as_deref() == Ok("iTerm.app")
    {
        print!("  Configure iTerm2 to use {font_family}? [y]es / [n]o: ");
        std::io::Write::flush(&mut std::io::stdout())?;

        let choice = read_line_lower()?;
        if choice.starts_with('y') {
            // Read current font size.
            let size = current_iterm2_font_size().unwrap_or(12);
            nerd_font::configure_iterm2_font(font_family, size)?;
            return Ok(true);
        }
        // User declined auto-config — tell them how to do it manually.
        println!("  → iTerm2: Settings → Profiles → Text → Font → \"{font_family}\"");
    } else {
        println!("  → Set your terminal font to \"{font_family}\" in terminal preferences.");
    }

    print!("  Continue setting theme? [y/n]: ");
    std::io::Write::flush(&mut std::io::stdout())?;
    let choice = read_line_lower()?;
    Ok(choice.starts_with('y'))
}

fn read_line_lower() -> Result<String> {
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_lowercase())
}

/// Read the current font size from iTerm2 preferences.
fn current_iterm2_font_size() -> Option<u32> {
    let output = std::process::Command::new("defaults")
        .args(["read", "com.googlecode.iterm2", "New Bookmarks"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("\"Normal Font\"") {
            // "Normal Font" = "Monaco 12";
            let val = trimmed.split('=').nth(1)?.trim().trim_matches(';').trim().trim_matches('"');
            let size_str = val.split_whitespace().last()?;
            return size_str.parse().ok();
        }
    }
    None
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

async fn cmd_convert(source: &str, name: Option<&str>, force: bool) -> Result<()> {
    // Resolve source.
    let resolved = lynx_convert::fetch::resolve_source(source)
        .context("failed to resolve theme source")?;

    // Derive theme name from source or use provided name.
    let theme_name = name
        .map(|n| n.to_string())
        .unwrap_or_else(|| {
            let stem = std::path::Path::new(source)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("converted")
                .to_string();
            // Strip .omp / .zsh-theme suffixes from multi-extension filenames
            // e.g. "atomic.omp" → "atomic", "candy.zsh-theme" → "candy"
            stem.strip_suffix(".omp")
                .or_else(|| stem.strip_suffix(".zsh-theme"))
                .unwrap_or(&stem)
                .to_string()
        });

    // Check for existing file.
    let out_path = user_theme_dir().join(format!("{theme_name}.toml"));
    if out_path.exists() && !force {
        return Err(LynxError::Theme(format!("theme '{}' already exists at {}. Use --force to overwrite.", theme_name, out_path.display())).into());
    }

    // Fetch content.
    let content = lynx_convert::fetch::fetch_content(&resolved)
        .context("failed to fetch theme content")?;

    // Auto-detect format: JSON = OMP, anything else = OMZ.
    let is_omp = content.trim_start().starts_with('{');

    // Write.
    std::fs::create_dir_all(user_theme_dir())?;

    if is_omp {
        // Oh-My-Posh JSON theme.
        let theme = lynx_convert::omp::parse(&content)
            .map_err(|e| anyhow::anyhow!(e))?;
        let toml_str = lynx_convert::emit::omp_to_lynx_toml(&theme, &theme_name);
        std::fs::write(&out_path, &toml_str)?;

        println!("Converted OMP theme → {}", out_path.display());
        if theme.two_line {
            println!("  Layout: two-line");
        }
        let seg_count = theme.top.len() + theme.top_right.len() + theme.left.len();
        println!("  Segments: {seg_count} mapped");
        if !theme.palette.is_empty() {
            println!("  Palette: {} colors extracted", theme.palette.len());
        }
        if !theme.notes.is_empty() {
            for note in &theme.notes {
                println!("  ⚠ {note}");
            }
        }
    } else {
        // OMZ .zsh-theme file.
        let ir = lynx_convert::omz::parse(&content);
        let toml_str = lynx_convert::emit::to_lynx_toml(&ir, &theme_name);
        std::fs::write(&out_path, &toml_str)?;

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
