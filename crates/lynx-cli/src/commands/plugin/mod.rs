// Plugin command — CLI args, dispatch, and simple config-mutation subcommands.
//
// More complex subcommands are split into focused submodules:
//   scaffold.rs     — `lx plugin new` (template generation)
//   registry_ops.rs — search, info, update, checksum, index-validate
//   shell_glue.rs   — exec and unload (eval-bridge script generation)

use anyhow::{bail, Result};
use clap::{Args, Subcommand};
use lynx_config::{load as load_config, snapshot::mutate_config_transaction};

mod registry_ops;
mod scaffold;
mod shell_glue;

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub struct PluginArgs {
    #[command(subcommand)]
    pub command: PluginCommand,
}

#[derive(Subcommand)]
pub enum PluginCommand {
    /// Install a plugin from a local path
    Add {
        /// Local path to the plugin directory
        path: String,
    },
    /// Remove an installed plugin
    Remove {
        /// Plugin name to remove
        name: String,
    },
    /// List installed plugins and their status
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Scaffold a new plugin directory
    New {
        /// Name for the new plugin
        name: String,
    },
    /// Remove and re-add a plugin
    Reinstall {
        /// Plugin name to reinstall
        name: String,
    },
    /// Generate shell activation glue for a loaded plugin (called by eval-bridge)
    Exec {
        /// Plugin name to exec
        name: String,
    },
    /// Unload a plugin from the shell (called by eval-bridge on profile switch)
    Unload {
        /// Plugin name to unload
        name: String,
    },
    /// Show real-world usage examples
    Examples,
    /// Search the plugin registry
    Search {
        /// Search query (fuzzy match on name and description)
        query: String,
        /// Refresh the registry index before searching
        #[arg(long)]
        refresh: bool,
    },
    /// Show full details for a registry plugin
    Info {
        /// Plugin name
        name: String,
    },
    /// Update installed plugin(s) to latest registry version
    Update {
        /// Plugin name (omit for --all)
        name: Option<String>,
        /// Update all registry-installed plugins
        #[arg(long)]
        all: bool,
    },
    /// Verify installed plugin checksum against lynx.lock
    Checksum {
        /// Plugin name or path to archive/file
        target: String,
    },
    /// Validate a registry index TOML file
    IndexValidate {
        /// Path to index.toml
        path: String,
    },
}

pub async fn run(args: PluginArgs) -> Result<()> {
    match args.command {
        PluginCommand::Add { path } => cmd_add(&path).await,
        PluginCommand::Remove { name } => cmd_remove(&name).await,
        PluginCommand::List { json } => cmd_list(json).await,
        PluginCommand::New { name } => scaffold::cmd_new(&name).await,
        PluginCommand::Reinstall { name } => registry_ops::cmd_reinstall(&name).await,
        PluginCommand::Exec { name } => shell_glue::cmd_exec(&name).await,
        PluginCommand::Unload { name } => shell_glue::cmd_unload(&name).await,
        PluginCommand::Search { query, refresh } => registry_ops::cmd_search(&query, refresh).await,
        PluginCommand::Info { name } => registry_ops::cmd_info(&name).await,
        PluginCommand::Update { name, all } => registry_ops::cmd_update(name.as_deref(), all).await,
        PluginCommand::Checksum { target } => registry_ops::cmd_checksum(&target).await,
        PluginCommand::IndexValidate { path } => registry_ops::cmd_index_validate(&path).await,
        PluginCommand::Examples => {
            crate::commands::examples::run(crate::commands::examples::ExamplesArgs {
                command: Some("plugin".into()),
            })
            .await
        }
    }
}

async fn cmd_add(path: &str) -> Result<()> {
    // Registry install: no path separators means it's a registry name, not a local path.
    if !path.contains('/') && !path.contains('\\') {
        return registry_ops::cmd_add_from_registry(path, false).await;
    }

    let plugin_path = std::path::PathBuf::from(path);
    let manifest_path = plugin_path.join(lynx_core::brand::PLUGIN_MANIFEST);

    if !manifest_path.exists() {
        bail!("no plugin.toml found at {}", manifest_path.display());
    }

    let content = std::fs::read_to_string(&manifest_path)?;
    let manifest =
        lynx_manifest::parse_and_validate(&content).map_err(|e| anyhow::anyhow!("{}", e))?;

    let name = manifest.plugin.name.clone();
    let config = load_config()?;

    if config.enabled_plugins.contains(&name) {
        println!("Plugin '{}' is already installed.", name);
        return Ok(());
    }

    mutate_config_transaction(&format!("plugin-add-{name}"), |cfg| {
        if !cfg.enabled_plugins.contains(&name) {
            cfg.enabled_plugins.push(name.clone());
        }
        Ok(())
    })?;
    println!("Added plugin '{}'.", name);
    Ok(())
}

async fn cmd_remove(name: &str) -> Result<()> {
    let config = load_config()?;
    if !config.enabled_plugins.iter().any(|p| p == name) {
        bail!("plugin '{}' is not installed.", name);
    }

    mutate_config_transaction(&format!("plugin-remove-{name}"), |cfg| {
        cfg.enabled_plugins.retain(|p| p != name);
        Ok(())
    })?;
    println!("Removed plugin '{}'.", name);
    Ok(())
}

async fn cmd_list(json: bool) -> Result<()> {
    let config = load_config()?;
    let context_str = format!("{:?}", config.active_context).to_lowercase();

    if json {
        let plugins: Vec<serde_json::Value> = config
            .enabled_plugins
            .iter()
            .map(|p| serde_json::json!({ "name": p, "context": context_str, "status": "enabled" }))
            .collect();
        println!("{}", serde_json::to_string_pretty(&plugins)?);
    } else if config.enabled_plugins.is_empty() {
        println!("No plugins installed.");
    } else {
        println!("{:<20} {:<12} CONTEXT", "NAME", "STATUS");
        println!("{}", "-".repeat(44));
        for p in &config.enabled_plugins {
            println!("{:<20} {:<12} {}", p, "enabled", context_str);
        }
    }
    Ok(())
}
