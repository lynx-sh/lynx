use std::process::Command;

use anyhow::{bail, Result};
use clap::{Args, Subcommand};

use lynx_config::{config_path, load, save};
use lynx_config::snapshot::create as snapshot;
use lynx_core::redact;

#[derive(Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Print current config (secrets redacted)
    Show,
    /// Open config in $EDITOR, validate before saving
    Edit,
    /// Validate config and report errors with line numbers
    Validate,
    /// Get a single config value by key (dot-notation)
    Get { key: String },
    /// Set a config value (snapshot → validate → apply)
    Set { key: String, value: String },
    /// Show real-world usage examples
    Examples,
}

pub async fn run(args: ConfigArgs) -> Result<()> {
    match args.command {
        ConfigCommand::Show => cmd_show(),
        ConfigCommand::Edit => cmd_edit(),
        ConfigCommand::Validate => cmd_validate(),
        ConfigCommand::Get { key } => cmd_get(&key),
        ConfigCommand::Set { key, value } => cmd_set(&key, &value),
        ConfigCommand::Examples => {
            return crate::commands::examples::run(
                crate::commands::examples::ExamplesArgs { command: Some("config".into()) }
            ).await;
        }
    }
}

fn cmd_show() -> Result<()> {
    let path = config_path();
    let content = std::fs::read_to_string(&path).unwrap_or_else(|_| "(no config file)".to_string());
    println!("{}", redact(&content));
    Ok(())
}

fn cmd_edit() -> Result<()> {
    let path = config_path();
    let snapshot_dir = snapshot(&path.parent().unwrap_or(&path), "config-edit")?;
    let _ = snapshot_dir;

    let snapshot_content = std::fs::read_to_string(&path).unwrap_or_default();
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

    let status = Command::new(&editor)
        .arg(&path)
        .status()
        .map_err(|e| anyhow::anyhow!("failed to launch editor '{editor}': {e}"))?;

    if !status.success() {
        std::fs::write(&path, &snapshot_content).ok();
        bail!("editor exited with error — config unchanged");
    }

    // Validate.
    match load() {
        Ok(_) => println!("config saved and validated"),
        Err(e) => {
            std::fs::write(&path, &snapshot_content)
                .map_err(|_| anyhow::anyhow!("CRITICAL: failed to restore config snapshot"))?;
            bail!("config validation failed — rolled back: {e}");
        }
    }
    Ok(())
}

fn cmd_validate() -> Result<()> {
    let path = config_path();
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => bail!("cannot read config: {e}"),
    };

    // Validate TOML parse.
    match toml::from_str::<toml::Value>(&content) {
        Ok(_) => {}
        Err(e) => {
            // toml errors include line/col info.
            bail!("TOML parse error: {e}");
        }
    }

    // Validate schema.
    match load() {
        Ok(_) => println!("config is valid"),
        Err(e) => bail!("schema validation error: {e}"),
    }
    Ok(())
}

fn cmd_get(key: &str) -> Result<()> {
    let cfg = load()?;
    let value = match key {
        "active_theme" => cfg.active_theme.clone(),
        "active_context" => format!("{:?}", cfg.active_context).to_lowercase(),
        "schema_version" => cfg.schema_version.to_string(),
        "sync.remote" => cfg.sync.remote.clone().unwrap_or_default(),
        other => bail!("unknown config key '{}' — known: active_theme, active_context, schema_version, sync.remote", other),
    };
    println!("{value}");
    Ok(())
}

fn cmd_set(key: &str, value: &str) -> Result<()> {
    let path = config_path();
    let config_dir = path.parent().unwrap_or(&path);
    snapshot(config_dir, &format!("config-set-{key}"))?;

    let mut cfg = load()?;
    match key {
        "active_theme" => {
            // Validate theme exists before applying.
            lynx_theme::loader::load(value)
                .map_err(|e| anyhow::anyhow!("theme not found: {e}"))?;
            cfg.active_theme = value.to_string();
        }
        "active_context" => {
            cfg.active_context = match value {
                "interactive" => lynx_core::types::Context::Interactive,
                "agent" => lynx_core::types::Context::Agent,
                "minimal" => lynx_core::types::Context::Minimal,
                other => bail!("unknown context '{other}'"),
            };
        }
        "sync.remote" => {
            cfg.sync.remote = if value.is_empty() { None } else { Some(value.to_string()) };
        }
        other => bail!("unknown config key '{}' — cannot set via CLI", other),
    }

    save(&cfg)?;
    println!("{key} = {value}");
    Ok(())
}

