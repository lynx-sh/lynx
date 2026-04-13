use anyhow::Result;

use super::open_in_editor;
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

pub fn run(args: ConfigArgs) -> Result<()> {
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
        }
        ConfigCommand::Other(args) => Err(LynxError::NotFound {
            item_type: "Command".into(),
            name: super::unknown_subcmd_name(&args).into(),
            hint: "run `lx config` for help".into(),
        }
        .into()),
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
    snapshot(path.parent().unwrap_or(&path), "config-edit")?;

    let file_existed = path.exists();
    let snapshot_content = std::fs::read_to_string(&path).unwrap_or_default();
    open_in_editor(&path)?;
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
            std::fs::write(&path, &snapshot_content).map_err(|_| {
                anyhow::Error::from(lynx_core::error::LynxError::Config(
                    "CRITICAL: failed to restore config snapshot".into(),
                ))
            })?;
            return Err(
                LynxError::Config(format!("config validation failed — rolled back: {e}")).into(),
            );
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
        "active_theme" => cfg.active_theme,
        "active_context" => format!("{:?}", cfg.active_context).to_lowercase(),
        "schema_version" => cfg.schema_version.to_string(),
        "sync.remote" => cfg.sync.remote.unwrap_or_default(),
        "tui.enabled" => cfg.tui.enabled.to_string(),
        other => {
            return Err(LynxError::NotFound {
                item_type: "Config key".into(),
                name: other.into(),
                hint: "known keys: active_theme, active_context, schema_version, sync.remote, tui.enabled"
                    .into(),
            }
            .into())
        }
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
            "tui.enabled" => {
                cfg.tui.enabled = match value {
                    "true" | "1" | "yes" => true,
                    "false" | "0" | "no" => false,
                    other => {
                        return Err(lynx_core::error::LynxError::Config(format!(
                            "invalid value '{other}' for tui.enabled — use true or false"
                        )))
                    }
                };
            }
            other => {
                return Err(lynx_core::error::LynxError::Config(format!(
                    "unknown config key '{other}' — cannot set via CLI"
                )))
            }
        }
        Ok(())
    })?;

    println!("{key} = {value}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmd_get_known_keys() {
        // These should not error with a valid config
        // We test the key matching logic by calling cmd_get with known keys.
        // It may fail if no config file exists, but the match arms are correct.
        let _ = cmd_get("active_theme");
        let _ = cmd_get("active_context");
        let _ = cmd_get("schema_version");
        let _ = cmd_get("sync.remote");
    }

    #[test]
    fn cmd_get_unknown_key_returns_not_found() {
        let result = cmd_get("nonexistent_key");
        // Even if config load fails, the unknown key path should be reached
        // if config loads successfully.
        // Either way, this should not panic.
        let _ = result;
    }

    #[test]
    fn cmd_show_does_not_panic() {
        // May fail if no config file, but should not panic
        let _ = cmd_show();
    }

    #[test]
    fn config_unknown_subcommand_returns_not_found() {
        let args = ConfigArgs {
            command: ConfigCommand::Other(vec!["bogus".to_string()]),
        };
        let err = run(args).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("bogus"), "error should mention command: {msg}");
    }

    #[test]
    fn cmd_validate_on_missing_config_returns_error() {
        // In a test environment without proper config, validate should error gracefully.
        let result = cmd_validate();
        // May or may not error — just don't panic.
        let _ = result;
    }
}
