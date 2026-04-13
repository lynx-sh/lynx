use anyhow::Result;

use super::open_in_vscode;
use clap::{Args, Subcommand};

use lynx_config::snapshot::{create as snapshot, mutate_config_transaction};
use lynx_config::{config_path, load};
use lynx_core::error::LynxError;
use lynx_core::redact;

#[derive(Args)]
#[command(arg_required_else_help = true)]
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
    /// Catch unknown subcommands for friendly error
    #[command(external_subcommand)]
    Other(Vec<String>),
}

pub async fn run(args: ConfigArgs) -> Result<()> {
    match args.command {
        ConfigCommand::Show => cmd_show(),
        ConfigCommand::Edit => cmd_edit(),
        ConfigCommand::Validate => cmd_validate(),
        ConfigCommand::Get { key } => cmd_get(&key),
        ConfigCommand::Set { key, value } => cmd_set(&key, &value),
        ConfigCommand::Examples => {
            crate::commands::examples::run(crate::commands::examples::ExamplesArgs {
                command: Some("config".into()),
            })
            .await
        }
        ConfigCommand::Other(args) => {
            Err(LynxError::NotFound {
                item_type: "Command".into(),
                name: args.first().map(|s| s.as_str()).unwrap_or("").into(),
                hint: "run `lx config` for help".into(),
            }.into())
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
    let snapshot_dir = snapshot(path.parent().unwrap_or(&path), "config-edit")?;
    let _ = snapshot_dir;

    let file_existed = path.exists();
    let snapshot_content = std::fs::read_to_string(&path).unwrap_or_default();
    open_in_vscode(&path)?;
    // VS Code edits in place; re-read to check for changes.
    let after = std::fs::read_to_string(&path).unwrap_or_default();
    if after == snapshot_content && (file_existed || after.is_empty()) {
        println!("no changes made");
        return Ok(());
    }

    // Validate.
    match load() {
        Ok(_) => println!("config saved and validated"),
        Err(e) => {
            std::fs::write(&path, &snapshot_content)
                .map_err(|_| anyhow::anyhow!("CRITICAL: failed to restore config snapshot"))?;
            return Err(LynxError::Config(format!("config validation failed — rolled back: {e}")).into());
        }
    }
    Ok(())
}

fn cmd_validate() -> Result<()> {
    let path = config_path();
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => return Err(LynxError::Config(format!("cannot read config: {e}")).into()),
    };

    // Validate TOML parse.
    match toml::from_str::<toml::Value>(&content) {
        Ok(_) => {}
        Err(e) => {
            // toml errors include line/col info.
            return Err(LynxError::Config(format!("TOML parse error: {e}")).into());
        }
    }

    // Validate schema.
    match load() {
        Ok(_) => println!("config is valid"),
        Err(e) => return Err(LynxError::Config(format!("schema validation error: {e}")).into()),
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
        other => return Err(LynxError::NotFound {
            item_type: "Config key".into(),
            name: other.into(),
            hint: "known keys: active_theme, active_context, schema_version, sync.remote".into(),
        }.into()),
    };
    println!("{value}");
    Ok(())
}

fn cmd_set(key: &str, value: &str) -> Result<()> {
    mutate_config_transaction(&format!("config-set-{key}"), |cfg| {
        match key {
            "active_theme" => {
                // Validate theme exists before applying.
                lynx_theme::loader::load(value).map_err(|e| {
                    lynx_core::error::LynxError::Config(format!("theme not found: {e}"))
                })?;
                cfg.active_theme = value.to_string();
            }
            "active_context" => {
                cfg.active_context = match value {
                    "interactive" => lynx_core::types::Context::Interactive,
                    "agent" => lynx_core::types::Context::Agent,
                    "minimal" => lynx_core::types::Context::Minimal,
                    other => {
                        return Err(lynx_core::error::LynxError::Config(format!(
                            "unknown context '{other}'"
                        )))
                    }
                };
            }
            "sync.remote" => {
                cfg.sync.remote = if value.is_empty() {
                    None
                } else {
                    Some(value.to_string())
                };
            }
            other => {
                return Err(lynx_core::error::LynxError::Config(format!(
                    "unknown config key '{}' — cannot set via CLI",
                    other
                )))
            }
        }
        Ok(())
    })?;

    println!("{key} = {value}");
    Ok(())
}
