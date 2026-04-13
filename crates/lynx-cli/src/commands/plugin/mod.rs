// Plugin command — CLI args, dispatch, and simple config-mutation subcommands.
//
// More complex subcommands are split into focused submodules:
//   scaffold.rs     — `lx plugin new` (template generation)
//   registry_ops.rs — search, info, update, checksum, index-validate
//   shell_glue.rs   — exec and unload (eval-bridge script generation)

use anyhow::Result;
use clap::{Args, Subcommand};
use lynx_config::{load as load_config, snapshot::mutate_config_transaction};
use lynx_core::error::LynxError;

mod path;
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
    /// Install a plugin — from the registry by name, or from a local path
    Add {
        /// Plugin name (registry) or path to a local plugin directory
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
    /// Unload a plugin from the shell
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
    /// Enable an installed plugin (add to enabled_plugins without reinstalling)
    Enable {
        /// Plugin name to enable
        name: String,
    },
    /// Disable a plugin without removing its files
    Disable {
        /// Plugin name to disable
        name: String,
    },
    /// Catch unknown subcommands for friendly error
    #[command(external_subcommand)]
    Other(Vec<String>),
}

pub async fn run(args: PluginArgs) -> Result<()> {
    match args.command {
        PluginCommand::Add { path } => cmd_add(&path).await,
        PluginCommand::Remove { name } => cmd_remove(&name),
        PluginCommand::List { json } => cmd_list(json),
        PluginCommand::New { name } => scaffold::cmd_new(&name),
        PluginCommand::Reinstall { name } => registry_ops::cmd_reinstall(&name).await,
        PluginCommand::Exec { name } => shell_glue::cmd_exec(&name).await,
        PluginCommand::Unload { name } => shell_glue::cmd_unload(&name),
        PluginCommand::Search { query, refresh } => registry_ops::cmd_search(&query, refresh).await,
        PluginCommand::Info { name } => registry_ops::cmd_info(&name).await,
        PluginCommand::Update { name, all } => registry_ops::cmd_update(name.as_deref(), all).await,
        PluginCommand::Checksum { target } => registry_ops::cmd_checksum(&target),
        PluginCommand::IndexValidate { path } => registry_ops::cmd_index_validate(&path),
        PluginCommand::Enable { name } => cmd_enable(&name),
        PluginCommand::Disable { name } => cmd_disable(&name),
        PluginCommand::Examples => {
            crate::commands::examples::run(crate::commands::examples::ExamplesArgs {
                command: Some("plugin".into()),
            })
        }
        PluginCommand::Other(args) => Err(LynxError::unknown_command(
            args.first().map(|s| s.as_str()).unwrap_or(""),
            "plugin",
        )
        .into()),
    }
}

async fn cmd_add(path: &str) -> Result<()> {
    // Registry install: no path separators means it's a name, not a local path.
    // Check installed dir and in-repo plugins/ first (bundled plugins) before hitting registry.
    if !path.contains('/') && !path.contains('\\') {
        let name = path;
        if path::resolve_plugin_dir(name)
            .map(|dir| dir.join(lynx_core::brand::PLUGIN_MANIFEST).exists())
            .unwrap_or(false)
        {
            return cmd_enable(name);
        }
        return registry_ops::cmd_add_from_registry(name, false).await;
    }

    let plugin_path = std::path::PathBuf::from(path);
    let manifest_path = plugin_path.join(lynx_core::brand::PLUGIN_MANIFEST);

    if !manifest_path.exists() {
        return Err(LynxError::Manifest(format!(
            "no plugin.toml found at {}",
            manifest_path.display()
        ))
        .into());
    }

    let content = std::fs::read_to_string(&manifest_path)?;
    let manifest = lynx_manifest::parse_and_validate(&content)
        .map_err(|e| anyhow::Error::from(lynx_core::error::LynxError::Manifest(e.to_string())))?;

    let name = manifest.plugin.name;
    let config = load_config()?;

    if config.enabled_plugins.contains(&name) {
        println!("Plugin '{name}' is already installed.");
        return Ok(());
    }

    mutate_config_transaction(&format!("plugin-add-{name}"), |cfg| {
        if !cfg.enabled_plugins.contains(&name) {
            cfg.enabled_plugins.push(name.clone());
        }
        Ok(())
    })?;
    println!("Added plugin '{name}'.");
    Ok(())
}

fn cmd_remove(name: &str) -> Result<()> {
    let config = load_config()?;
    if !config.enabled_plugins.iter().any(|p| p == name) {
        return Err(LynxError::NotInstalled(name.to_string()).into());
    }

    mutate_config_transaction(&format!("plugin-remove-{name}"), |cfg| {
        cfg.enabled_plugins.retain(|p| p != name);
        Ok(())
    })?;
    println!("Removed plugin '{name}'.");
    Ok(())
}

fn cmd_enable(name: &str) -> Result<()> {
    let config = load_config()?;
    if config.enabled_plugins.iter().any(|p| p == name) {
        println!("Plugin '{name}' is already enabled.");
        return Ok(());
    }
    lynx_config::enable_plugin(name)?;
    println!("Enabled plugin '{name}'. Restart your shell to activate.");
    Ok(())
}

fn cmd_disable(name: &str) -> Result<()> {
    lynx_config::disable_plugin(name)?;
    println!("Disabled plugin '{name}'. Restart your shell to take effect.");
    Ok(())
}

struct PluginListEntry {
    name: String,
    context: String,
}

impl lynx_tui::ListItem for PluginListEntry {
    fn title(&self) -> &str {
        &self.name
    }
    fn subtitle(&self) -> String {
        "enabled".to_string()
    }
    fn detail(&self) -> String {
        format!("Status: enabled\nContext: {}", self.context)
    }
    fn category(&self) -> Option<&str> {
        Some("plugin")
    }
    fn is_active(&self) -> bool {
        true
    }
}

fn cmd_list(json: bool) -> Result<()> {
    let config = load_config()?;
    let context_str = format!("{:?}", config.active_context).to_lowercase();

    if json {
        let plugins: Vec<serde_json::Value> = config
            .enabled_plugins
            .iter()
            .map(|p| serde_json::json!({ "name": p, "context": context_str, "status": "enabled" }))
            .collect();
        println!("{}", serde_json::to_string_pretty(&plugins)?);
        return Ok(());
    }

    if config.enabled_plugins.is_empty() {
        println!("No plugins installed.");
        return Ok(());
    }

    let entries: Vec<PluginListEntry> = config
        .enabled_plugins
        .iter()
        .map(|p| PluginListEntry {
            name: p.clone(),
            context: context_str.clone(),
        })
        .collect();

    let tui_colors = match lynx_theme::loader::load(&config.active_theme) {
        Ok(theme) => lynx_tui::TuiColors::from_palette(&theme.colors),
        Err(_) => lynx_tui::TuiColors::default(),
    };

    if let Some(idx) = lynx_tui::show(&entries, "Plugins", &tui_colors)? {
        let name = &entries[idx].name;
        println!("  Disable '{name}'? Use: lx plugin disable {name}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_list_entry_trait() {
        use lynx_tui::ListItem;
        let entry = PluginListEntry {
            name: "git".to_string(),
            context: "interactive".to_string(),
        };
        assert_eq!(entry.title(), "git");
        assert_eq!(entry.subtitle(), "enabled");
        assert!(entry.is_active());
        assert_eq!(entry.category(), Some("plugin"));
        assert!(entry.detail().contains("interactive"));
    }

    #[tokio::test]
    async fn plugin_unknown_subcommand_errors() {
        let args = PluginArgs {
            command: PluginCommand::Other(vec!["nope".to_string()]),
        };
        let err = run(args).await.unwrap_err();
        assert!(err.to_string().contains("nope"));
    }

    #[test]
    fn cmd_add_path_detection() {
        // Paths with / are treated as local paths, names without are registry lookups
        assert!("./my-plugin".contains('/'));
        assert!("/abs/path".contains('/'));
        assert!(!"my-plugin".contains('/'));
    }
}
