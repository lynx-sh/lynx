use anyhow::{bail, Context as _, Result};

use super::open_in_vscode;
use clap::{Args, Subcommand};

use lynx_config::{load, snapshot::mutate_config_transaction};
use lynx_intro::{
    figlet,
    loader::{self, user_intro_dir},
};

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub struct IntroArgs {
    #[command(subcommand)]
    pub command: IntroCommand,
}

#[derive(Subcommand)]
pub enum IntroCommand {
    /// Enable intro display at shell startup
    On,
    /// Disable intro display at shell startup
    Off,
    /// Set the active intro by slug
    Set { slug: String },
    /// List all available intros (built-in and user-defined)
    List,
    /// Open an intro in $EDITOR; validates on save and rolls back on error
    Edit { slug: String },
    /// Delete a user-defined intro (built-in intros cannot be deleted)
    Delete { slug: String },
    /// Scaffold a new intro file and open it in $EDITOR
    New { slug: String },
    /// Preview the rendered intro in the terminal
    Preview {
        /// Slug to preview (defaults to the active intro)
        slug: Option<String>,
    },
    /// Smart dispatch: treat unknown subcommand as slug for `set`
    #[command(external_subcommand)]
    Other(Vec<String>),
    /// Generate ASCII art from a bundled figlet font and print to stdout
    Logo {
        /// Text to render
        text: String,
        /// Font to use (default: slant)
        #[arg(long, default_value = "slant")]
        font: String,
        /// List available fonts and exit
        #[arg(long)]
        list_fonts: bool,
        /// Append the generated logo to the active intro's first AsciiLogo block (or prepend one)
        #[arg(long)]
        append: bool,
    },
}

pub async fn run(args: IntroArgs) -> Result<()> {
    match args.command {
        IntroCommand::On => cmd_on(),
        IntroCommand::Off => cmd_off(),
        IntroCommand::Set { slug } => cmd_set(&slug),
        IntroCommand::List => cmd_list(),
        IntroCommand::Edit { slug } => cmd_edit(&slug),
        IntroCommand::Delete { slug } => cmd_delete(&slug),
        IntroCommand::New { slug } => cmd_new(&slug),
        IntroCommand::Preview { slug } => cmd_preview(slug.as_deref()),
        IntroCommand::Other(args) => {
            if args.len() == 1 {
                cmd_set(&args[0])
            } else {
                bail!("unknown intro command '{}' — run `lx intro` for help", args.first().map(|s| s.as_str()).unwrap_or(""))
            }
        }
        IntroCommand::Logo { text, font, list_fonts, append } => {
            cmd_logo(&text, &font, list_fonts, append)
        }
    }
}

fn cmd_on() -> Result<()> {
    mutate_config_transaction("intro-on", |cfg| {
        cfg.intro.enabled = true;
        Ok(())
    })
    .context("failed to save config")?;
    println!("intro enabled — use `lx intro set <slug>` to choose one");
    Ok(())
}

fn cmd_off() -> Result<()> {
    mutate_config_transaction("intro-off", |cfg| {
        cfg.intro.enabled = false;
        Ok(())
    })
    .context("failed to save config")?;
    println!("intro disabled");
    Ok(())
}

fn cmd_set(slug: &str) -> Result<()> {
    // Validate slug exists before mutating config.
    loader::load(slug).with_context(|| format!("intro '{slug}' not found"))?;

    mutate_config_transaction(&format!("intro-set-{slug}"), |cfg| {
        cfg.intro.active = Some(slug.to_string());
        Ok(())
    })
    .context("failed to save config")?;

    println!("active intro set to '{slug}'");
    Ok(())
}

/// An intro entry for the interactive list.
struct IntroListEntry {
    slug: String,
    name: String,
    kind: String,
    is_current: bool,
    enabled: bool,
}

impl lynx_tui::ListItem for IntroListEntry {
    fn title(&self) -> &str {
        &self.slug
    }

    fn subtitle(&self) -> String {
        let status = if self.is_current && !self.enabled {
            " (disabled)"
        } else {
            ""
        };
        format!("{}{status}", self.kind)
    }

    fn detail(&self) -> String {
        let mut lines = vec![self.name.clone()];
        lines.push(format!("Type: {}", self.kind));
        if self.is_current {
            let status = if self.enabled { "active" } else { "active (disabled)" };
            lines.push(format!("Status: {status}"));
        }
        lines.join("\n")
    }

    fn category(&self) -> Option<&str> {
        Some("intro")
    }

    fn is_active(&self) -> bool {
        self.is_current
    }
}

fn cmd_list() -> Result<()> {
    let cfg = load().context("failed to load config")?;
    let active = cfg.intro.active.as_deref().unwrap_or("");
    let enabled = cfg.intro.enabled;

    let raw_entries = loader::list_all();
    if raw_entries.is_empty() {
        println!("no intros found");
        return Ok(());
    }

    let entries: Vec<IntroListEntry> = raw_entries
        .iter()
        .map(|e| IntroListEntry {
            slug: e.slug.clone(),
            name: e.name.clone(),
            kind: if e.is_builtin { "built-in".into() } else { "user".into() },
            is_current: e.slug == active,
            enabled,
        })
        .collect();

    // Load TUI colors from active theme.
    let tui_colors = match lynx_theme::loader::load(&cfg.active_theme) {
        Ok(theme) => lynx_tui::TuiColors::from_palette(&theme.colors),
        Err(_) => lynx_tui::TuiColors::default(),
    };

    if let Some(idx) = lynx_tui::show(&entries, "Intros", &tui_colors)? {
        let selected = &entries[idx].slug;
        if selected != active {
            cmd_set(selected)?;
        }
    }

    Ok(())
}

fn cmd_edit(slug: &str) -> Result<()> {
    let user_dir = user_intro_dir();
    let user_path = user_dir.join(format!("{slug}.toml"));

    let path = if user_path.exists() {
        user_path
    } else {
        // Copy built-in to user dir first.
        copy_builtin_to_user(slug)?
    };

    let snapshot = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read intro file {path:?}"))?;

    open_in_vscode(&path)?;

    // Validate the saved file.
    match lynx_intro::loader::load_user(slug) {
        Ok(_) => {
            println!("intro '{slug}' saved and validated");
        }
        Err(e) => {
            std::fs::write(&path, &snapshot)
                .context("CRITICAL: failed to restore intro snapshot")?;
            bail!("intro validation failed — changes reverted: {e}");
        }
    }
    Ok(())
}

fn cmd_delete(slug: &str) -> Result<()> {
    // Only user intros can be deleted.
    if loader::list_builtin().contains(&slug) {
        bail!("cannot delete built-in intro '{slug}' — built-ins are read-only");
    }

    let path = user_intro_dir().join(format!("{slug}.toml"));
    if !path.exists() {
        bail!("intro '{slug}' not found in user intro directory");
    }

    std::fs::remove_file(&path)
        .with_context(|| format!("failed to delete intro '{slug}'"))?;

    // If this was the active intro, clear it from config.
    let cfg = load().context("failed to load config")?;
    if cfg.intro.active.as_deref() == Some(slug) {
        if let Err(e) = mutate_config_transaction("intro-clear-active", |cfg| {
            cfg.intro.active = None;
            Ok(())
        }) {
            lynx_core::diag::warn("intro", &format!("failed to clear active intro from config: {e}"));
            eprintln!("warning: could not clear active intro from config: {e}");
        }
    }

    println!("intro '{slug}' deleted");
    Ok(())
}

fn cmd_new(slug: &str) -> Result<()> {
    // Validate slug is a safe identifier.
    if slug.contains('/') || slug.contains('\\') || slug.contains("..") || slug.is_empty() {
        bail!("invalid slug '{slug}': use only letters, numbers, hyphens, and underscores");
    }

    let user_dir = user_intro_dir();
    std::fs::create_dir_all(&user_dir).context("failed to create user intro directory")?;

    let path = user_dir.join(format!("{slug}.toml"));
    if path.exists() {
        bail!("intro '{slug}' already exists — use `lx intro edit {slug}` to modify it");
    }

    let template = format!(
        r#"[meta]
name        = "{slug}"
description = ""
author      = ""

[display]
on_startup   = true
on_new_tab   = false
on_ssh       = true
cooldown_sec = 0

[[blocks]]
type    = "text"
content = "  Welcome back, {{{{username}}}}!"
color   = "yellow"
bold    = true

[[blocks]]
type  = "separator"
char  = "─"
width = 40
color = "muted"
"#
    );

    std::fs::write(&path, &template)
        .with_context(|| format!("failed to write new intro '{slug}'"))?;

    open_in_vscode(&path)?;

    // Validate after edit.
    match lynx_intro::loader::load_user(slug) {
        Ok(_) => println!("intro '{slug}' created"),
        Err(e) => {
            if let Err(rm_err) = std::fs::remove_file(&path) {
                lynx_core::diag::warn("intro", &format!("failed to clean up invalid intro file {path:?}: {rm_err}"));
                eprintln!("warning: could not remove invalid intro file: {rm_err}");
            }
            bail!("intro validation failed — file removed: {e}");
        }
    }
    Ok(())
}

fn cmd_preview(slug: Option<&str>) -> Result<()> {
    let cfg = load().context("failed to load config")?;
    let target = slug
        .or(cfg.intro.active.as_deref())
        .ok_or_else(|| anyhow::anyhow!("no intro specified and no active intro set — use `lx intro set <slug>` first"))?;

    let intro = loader::load(target)
        .with_context(|| format!("failed to load intro '{target}'"))?;

    let env: std::collections::HashMap<String, String> = std::env::vars().collect();
    let tokens = lynx_intro::build_token_map(&env);
    let rendered = lynx_intro::render_intro(&intro, &tokens);

    print!("{}", rendered);
    Ok(())
}

fn cmd_logo(text: &str, font: &str, list_fonts: bool, append: bool) -> Result<()> {
    if list_fonts {
        println!("Available fonts:");
        for f in figlet::list_fonts() {
            println!("  {f}");
        }
        return Ok(());
    }

    let ascii = figlet::render_ascii(font, text)
        .with_context(|| format!("failed to render ASCII art with font '{font}'"))?;

    if append {
        append_logo_to_active_intro(font, text)?;
        println!("logo appended to active intro");
    } else {
        print!("{}", ascii);
    }
    Ok(())
}

/// Append (or update) an AsciiLogo block in the active intro's user file.
fn append_logo_to_active_intro(font: &str, text: &str) -> Result<()> {
    let cfg = load().context("failed to load config")?;
    let slug = cfg.intro.active.as_deref()
        .ok_or_else(|| anyhow::anyhow!("no active intro — use `lx intro set <slug>` first"))?;

    // Ensure it's in user dir (copy built-in if needed).
    let user_dir = user_intro_dir();
    let path = user_dir.join(format!("{slug}.toml"));
    if !path.exists() {
        copy_builtin_to_user(slug)?;
    }

    let mut intro = lynx_intro::loader::load(slug)?;

    // Replace the first AsciiLogo block if it exists, otherwise prepend one.
    let new_block = lynx_intro::Block::AsciiLogo {
        font: font.to_string(),
        text: text.to_string(),
        color: None,
    };

    let mut replaced = false;
    for block in &mut intro.blocks {
        if matches!(block, lynx_intro::Block::AsciiLogo { .. }) {
            *block = new_block.clone();
            replaced = true;
            break;
        }
    }
    if !replaced {
        intro.blocks.insert(0, new_block);
    }

    let content = toml::to_string_pretty(&intro)
        .context("failed to serialize updated intro")?;
    std::fs::write(&path, content)
        .with_context(|| format!("failed to write intro '{slug}'"))?;

    Ok(())
}

/// Copy a built-in intro to the user intro directory and return the path.
fn copy_builtin_to_user(slug: &str) -> Result<std::path::PathBuf> {
    let intro = loader::load_builtin(slug)
        .ok_or_else(|| anyhow::anyhow!("intro '{slug}' not found in built-ins"))?;

    let user_dir = user_intro_dir();
    std::fs::create_dir_all(&user_dir)
        .context("failed to create user intro directory")?;

    let path = user_dir.join(format!("{slug}.toml"));
    let content = toml::to_string_pretty(&intro)
        .context("failed to serialize built-in intro")?;
    std::fs::write(&path, &content)
        .with_context(|| format!("failed to copy built-in intro '{slug}' to user dir"))?;

    Ok(path)
}
