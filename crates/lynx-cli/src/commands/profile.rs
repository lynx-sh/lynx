use anyhow::{bail, Result};
use clap::{Args, Subcommand};
use lynx_config::{
    load as load_config, save as save_config,
    profile::{self, Profile},
    profile_activator::{activate_profile, ActiveState},
    snapshot,
};
use std::path::PathBuf;

#[derive(Args)]
pub struct ProfileArgs {
    #[command(subcommand)]
    pub command: ProfileCommand,
}

#[derive(Subcommand)]
pub enum ProfileCommand {
    /// Create a new profile from template
    Create {
        /// Profile name
        name: String,
        /// Overwrite if already exists
        #[arg(long)]
        force: bool,
    },
    /// Open profile in $EDITOR and validate after save
    Edit {
        /// Profile name
        name: String,
    },
    /// Switch to a profile
    Switch {
        /// Profile name
        name: String,
    },
    /// List all profiles with active marker
    List,
    /// Delete a profile
    Delete {
        /// Profile name
        name: String,
    },
    /// Show the active profile details
    Show,
}

pub async fn run(args: ProfileArgs) -> Result<()> {
    match args.command {
        ProfileCommand::Create { name, force } => cmd_create(&name, force),
        ProfileCommand::Edit { name }          => cmd_edit(&name),
        ProfileCommand::Switch { name }        => cmd_switch(&name).await,
        ProfileCommand::List                   => cmd_list(),
        ProfileCommand::Delete { name }        => cmd_delete(&name),
        ProfileCommand::Show                   => cmd_show(),
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn profiles_dir() -> PathBuf {
    profile::profiles_dir()
}

fn profile_path(name: &str) -> PathBuf {
    profiles_dir().join(format!("{name}.toml"))
}

// ── create ───────────────────────────────────────────────────────────────────

fn cmd_create(name: &str, force: bool) -> Result<()> {
    validate_name(name)?;
    let path = profile_path(name);
    if path.exists() && !force {
        bail!("profile '{name}' already exists — use --force to overwrite");
    }
    std::fs::create_dir_all(profiles_dir())?;
    std::fs::write(&path, profile_template(name))?;
    println!("created: {}", path.display());
    println!("edit with: lx profile edit {name}");
    Ok(())
}

fn profile_template(name: &str) -> String {
    format!(
        r#"# Lynx profile: {name}
# Uncomment and edit the fields you want to use.

name    = "{name}"
# extends = "default"      # inherit from another profile (single level)
theme   = "default"
plugins = ["git", "fzf"]

# context_override = "interactive"   # suggested context — not enforced

[env]
# EDITOR    = "nvim"
# KUBECONFIG = "~/.kube/{name}-config"
# DO NOT store secrets here — use a secrets manager instead.

[aliases]
# Aliases are only loaded in interactive context.
# ll = "ls -la"
"#
    )
}

// ── edit ─────────────────────────────────────────────────────────────────────

fn cmd_edit(name: &str) -> Result<()> {
    let path = profile_path(name);
    if !path.exists() {
        bail!("profile '{name}' does not exist — create it with: lx profile create {name}");
    }

    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vi".into());

    let status = std::process::Command::new(&editor)
        .arg(&path)
        .status()?;

    if !status.success() {
        bail!("editor exited with non-zero status");
    }

    // Validate after save.
    match profile::load_from(&path) {
        Ok((_, warns)) => {
            for w in &warns {
                eprintln!("warning: {}", w.message);
            }
            println!("profile '{name}' is valid");
        }
        Err(e) => {
            bail!("profile '{name}' has errors after edit: {e}");
        }
    }
    Ok(())
}

// ── switch ───────────────────────────────────────────────────────────────────

async fn cmd_switch(name: &str) -> Result<()> {
    // Validate profile exists and resolves cleanly.
    let (resolved, warns) = profile::resolve(name)?;
    for w in &warns {
        eprintln!("warning: {}", w.message);
    }

    // Check plugin deps are satisfied.
    check_plugin_deps(&resolved)?;

    // Snapshot before mutating config (D-007).
    let config_dir = {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        home.join(".config/lynx")
    };
    snapshot::create(&config_dir, &format!("profile-switch-{name}"))
        .unwrap_or_default(); // non-fatal — warn only

    // Build current active state from config.
    let mut cfg = load_config()?;
    let current = active_state_from_config(&cfg);

    // Emit activation zsh.
    let zsh = activate_profile(&resolved, &current);
    if !zsh.is_empty() {
        println!("{zsh}");
    }

    // Persist active_profile in config.
    cfg.active_profile = Some(name.to_string());
    // Also sync theme/plugins from profile so config stays consistent.
    if let Some(ref t) = resolved.theme {
        cfg.active_theme = t.clone();
    }
    save_config(&cfg)?;

    eprintln!("switched to profile '{name}'");
    Ok(())
}

fn check_plugin_deps(profile: &Profile) -> Result<()> {
    // Best-effort: warn about plugins that don't have a plugin.toml installed.
    let plugin_base = {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        home.join(".local/share/lynx/plugins")
    };

    let mut missing = Vec::new();
    for p in &profile.plugins {
        let manifest = plugin_base.join(p).join("plugin.toml");
        if !manifest.exists() {
            missing.push(p.as_str());
        }
    }
    if !missing.is_empty() {
        eprintln!(
            "warning: profile references plugins not installed: {} — run: lx plugin add ./plugins/<name>",
            missing.join(", ")
        );
    }
    Ok(())
}

fn active_state_from_config(cfg: &lynx_config::schema::LynxConfig) -> ActiveState {
    ActiveState {
        plugins: cfg.enabled_plugins.clone(),
        theme: Some(cfg.active_theme.clone()),
        env: std::collections::HashMap::new(),
        aliases: std::collections::HashMap::new(),
    }
}

// ── list ─────────────────────────────────────────────────────────────────────

fn cmd_list() -> Result<()> {
    let cfg = load_config()?;
    let active = cfg.active_profile.as_deref();
    let names = profile::list_names()?;

    if names.is_empty() {
        println!("no profiles — create one with: lx profile create <name>");
        return Ok(());
    }

    for name in &names {
        let marker = if Some(name.as_str()) == active { "*" } else { " " };
        // Load for summary (plugin count + theme) — best effort.
        let summary = profile::load(name)
            .map(|(p, _)| {
                let theme = p.theme.as_deref().unwrap_or("default");
                format!("{} plugins, theme: {theme}", p.plugins.len())
            })
            .unwrap_or_else(|_| "error loading".into());
        println!("{marker} {name:<20} {summary}");
    }
    Ok(())
}

// ── delete ───────────────────────────────────────────────────────────────────

fn cmd_delete(name: &str) -> Result<()> {
    let cfg = load_config()?;
    if cfg.active_profile.as_deref() == Some(name) {
        bail!("cannot delete the active profile '{name}' — switch to another profile first");
    }
    let path = profile_path(name);
    if !path.exists() {
        bail!("profile '{name}' does not exist");
    }
    std::fs::remove_file(&path)?;
    println!("deleted profile '{name}'");
    Ok(())
}

// ── show ─────────────────────────────────────────────────────────────────────

fn cmd_show() -> Result<()> {
    let cfg = load_config()?;
    match cfg.active_profile.as_deref() {
        None => println!("no active profile"),
        Some(name) => {
            let (resolved, warns) = profile::resolve(name)?;
            for w in &warns {
                eprintln!("warning: {}", w.message);
            }
            println!("active profile: {name}");
            if let Some(ref ext) = resolved.extends {
                println!("  extends: {ext}");
            }
            println!("  theme:   {}", resolved.theme.as_deref().unwrap_or("default"));
            println!("  plugins: {}", resolved.plugins.join(", "));
            if !resolved.env.is_empty() {
                let keys: Vec<&str> = resolved.env.keys().map(|k| k.as_str()).collect();
                println!("  env:     {}", keys.join(", "));
            }
        }
    }
    Ok(())
}

// ── validation ───────────────────────────────────────────────────────────────

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!("profile name '{name}' is invalid — use only alphanumeric, dash, or underscore");
    }
    Ok(())
}
