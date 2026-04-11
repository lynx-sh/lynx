use anyhow::{bail, Result};
use clap::{Args, Subcommand};
use lynx_config::{load as load_config, save as save_config};
use lynx_plugin::exec::generate_exec_script;
use lynx_plugin::namespace::scaffold_convention_comment;
use lynx_registry::fetch::{check_for_update, fetch_plugin, update_plugin, FetchOptions};
use lynx_registry::index::{get_index, load_lock};
use std::path::PathBuf;

#[derive(Args)]
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
}

pub async fn run(args: PluginArgs) -> Result<()> {
    match args.command {
        PluginCommand::Add { path } => cmd_add(&path).await,
        PluginCommand::Remove { name } => cmd_remove(&name).await,
        PluginCommand::List { json } => cmd_list(json).await,
        PluginCommand::New { name } => cmd_new(&name).await,
        PluginCommand::Reinstall { name } => cmd_reinstall(&name).await,
        PluginCommand::Exec { name } => cmd_exec(&name).await,
        PluginCommand::Search { query, refresh } => cmd_search(&query, refresh).await,
        PluginCommand::Info { name } => cmd_info(&name).await,
        PluginCommand::Update { name, all } => cmd_update(name.as_deref(), all).await,
        PluginCommand::Examples => {
            crate::commands::examples::run(
                crate::commands::examples::ExamplesArgs { command: Some("plugin".into()) }
            ).await
        }
    }
}

async fn cmd_add(path: &str) -> Result<()> {
    // If path looks like a registry name (no path separators, no ./), fetch from registry.
    if !path.contains('/') && !path.contains('\\') {
        return cmd_add_from_registry(path, false).await;
    }

    let plugin_path = PathBuf::from(path);
    let manifest_path = plugin_path.join("plugin.toml");

    if !manifest_path.exists() {
        bail!("no plugin.toml found at {}", manifest_path.display());
    }

    let content = std::fs::read_to_string(&manifest_path)?;
    let manifest = lynx_manifest::parse_and_validate(&content)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let mut config = load_config()?;
    let name = manifest.plugin.name.clone();

    if config.enabled_plugins.contains(&name) {
        println!("Plugin '{}' is already installed.", name);
        return Ok(());
    }

    config.enabled_plugins.push(name.clone());
    save_config(&config)?;
    println!("Added plugin '{}'.", name);
    Ok(())
}

async fn cmd_add_from_registry(name: &str, force: bool) -> Result<()> {
    let install_dir = tokio::task::spawn_blocking({
        let name = name.to_string();
        move || fetch_plugin(&name, &FetchOptions { force, refresh_index: true, ..Default::default() })
    })
    .await??;

    let mut config = load_config()?;
    if !config.enabled_plugins.contains(&name.to_string()) {
        config.enabled_plugins.push(name.to_string());
        save_config(&config)?;
    }
    println!("installed '{}' to {}", name, install_dir.display());
    Ok(())
}

async fn cmd_remove(name: &str) -> Result<()> {
    let mut config = load_config()?;
    let before = config.enabled_plugins.len();
    config.enabled_plugins.retain(|p| p != name);
    if config.enabled_plugins.len() == before {
        bail!("plugin '{}' is not installed.", name);
    }
    save_config(&config)?;
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
    } else {
        if config.enabled_plugins.is_empty() {
            println!("No plugins installed.");
        } else {
            println!("{:<20} {:<12} {}", "NAME", "STATUS", "CONTEXT");
            println!("{}", "-".repeat(44));
            for p in &config.enabled_plugins {
                println!("{:<20} {:<12} {}", p, "enabled", context_str);
            }
        }
    }
    Ok(())
}

async fn cmd_new(name: &str) -> Result<()> {
    let dir = PathBuf::from(name);
    if dir.exists() {
        bail!("directory '{}' already exists.", name);
    }

    std::fs::create_dir_all(dir.join("shell"))?;

    // plugin.toml — every field has an inline comment explaining it
    let toml = format!(
        r#"[plugin]
name        = "{name}"   # unique identifier — must match the directory name
version     = "0.1.0"   # semver; bump when you make breaking changes
description = ""         # shown in `lx plugin list`
authors     = []         # e.g. ["Your Name <you@example.com>"]

[load]
lazy  = false  # true = load only on first use of an exported function
hooks = []     # zsh hooks that trigger load, e.g. ["chpwd", "precmd"]

[deps]
binaries = []  # required binaries, e.g. ["git", "fzf"] — checked at load
plugins  = []  # other lynx plugins this one depends on

[exports]
# List every function and alias exported to the shell.
# Unlisted names are private — Lynx will refuse to source them.
functions = ["{name}"]   # example: replace with your real function names
aliases   = []           # example: ["g", "gs"] — only loaded in interactive context

[contexts]
# Aliases are never loaded in agent or minimal contexts (D-010).
# Add "interactive" here to also skip functions in non-interactive shells.
disabled_in = ["agent", "minimal"]
"#,
        name = name
    );
    std::fs::write(dir.join("plugin.toml"), toml)?;

    // shell/init.zsh — thin entry point, sources the other files, under 10 lines
    let init_zsh = format!(
        "# {name} — init.zsh  (keep this file under 10 lines)\n\
         # Sources functions and aliases; actual logic lives in functions.zsh.\n\
         source \"${{LYNX_PLUGIN_DIR}}/{name}/shell/functions.zsh\"\n\
         source \"${{LYNX_PLUGIN_DIR}}/{name}/shell/aliases.zsh\"\n",
        name = name,
    );
    std::fs::write(dir.join("shell/init.zsh"), init_zsh)?;

    // shell/functions.zsh — example function with _ prefix for internals
    let functions_zsh = format!(
        "# {name} -- functions.zsh\n\
         # Public functions must match the exports.functions list in plugin.toml.\n\
         # Internal helpers use the _ prefix so Lynx won't export them.\n\
         \n\
         {convention}\n\
         \n\
         # Example public function -- rename and replace with your logic.\n\
         {name}() {{\n\
         {indent}__{name}_run \"$@\"\n\
         }}\n\
         \n\
         # Internal helper -- not exported.\n\
         __{name}_run() {{\n\
         {indent}echo \"{name}: $*\"\n\
         }}\n",
        name = name,
        convention = scaffold_convention_comment(),
        indent = "  ",
    );
    std::fs::write(dir.join("shell/functions.zsh"), functions_zsh)?;

    // shell/aliases.zsh — context-gated example
    let aliases_zsh = format!(
        "# {name} — aliases.zsh\n\
         # Aliases are only sourced in interactive context (disabled_in agent+minimal).\n\
         # All aliases must be listed in exports.aliases in plugin.toml.\n\
         \n\
         # Example alias — remove or replace:\n\
         # alias {short}='{name}'\n",
        name = name,
        short = name.chars().next().unwrap_or('x'),
    );
    std::fs::write(dir.join("shell/aliases.zsh"), aliases_zsh)?;

    println!("Created plugin '{}' at ./{}/", name, name);
    println!();
    println!("  Structure:");
    println!("    {name}/plugin.toml          — manifest (edit exports + deps)");
    println!("    {name}/shell/init.zsh        — entry point (keep under 10 lines)");
    println!("    {name}/shell/functions.zsh   — your functions go here");
    println!("    {name}/shell/aliases.zsh     — aliases (context-gated automatically)");
    println!();
    println!("  Next:");
    println!("    lx plugin add ./{name}       — install and activate");
    println!("    lx plugin list               — verify it's loaded");
    println!("    lx doctor                    — check for issues");
    Ok(())
}

async fn cmd_reinstall(name: &str) -> Result<()> {
    cmd_add_from_registry(name, true).await
}

async fn cmd_search(query: &str, refresh: bool) -> Result<()> {
    let idx = tokio::task::spawn_blocking({
        let refresh = refresh;
        move || get_index(refresh, None)
    })
    .await??;

    let results = idx.search(query);
    if results.is_empty() {
        println!("no results for '{query}'");
        return Ok(());
    }

    let lock = load_lock().unwrap_or_default();
    println!("{:<20} {:<10} {}", "NAME", "VERSION", "DESCRIPTION");
    println!("{}", "-".repeat(60));
    for entry in results {
        let installed = if lock.find(&entry.name).is_some() { "*" } else { " " };
        println!(
            "{installed}{:<19} {:<10} {}",
            entry.name, entry.latest_version, entry.description
        );
    }
    println!("\n* = installed   install: lx plugin add <name>");
    Ok(())
}

async fn cmd_info(name: &str) -> Result<()> {
    let idx = tokio::task::spawn_blocking(|| get_index(false, None)).await??;
    let entry = idx
        .find(name)
        .ok_or_else(|| anyhow::anyhow!("plugin '{name}' not found in registry"))?;

    let lock = load_lock().unwrap_or_default();
    let installed = lock.find(name);

    println!("name:        {}", entry.name);
    println!("description: {}", entry.description);
    println!("author:      {}", entry.author);
    println!("latest:      {}", entry.latest_version);
    println!("versions:    {}", entry.versions.len());
    for v in &entry.versions {
        let min = v.min_lynx_version.as_deref().unwrap_or("any");
        println!("  {} — min_lynx: {min}", v.version);
    }
    if let Some(locked) = installed {
        println!("installed:   v{}", locked.version);
        if locked.version != entry.latest_version {
            println!("             (update available: {})", entry.latest_version);
        }
    } else {
        println!("installed:   no   (lx plugin add {name})");
    }
    Ok(())
}

async fn cmd_update(name: Option<&str>, all: bool) -> Result<()> {
    if all {
        // Update all registry-installed plugins.
        let lock = load_lock().unwrap_or_default();
        let registry_names: Vec<String> = lock
            .entries
            .iter()
            .filter(|e| e.source == "registry")
            .map(|e| e.name.clone())
            .collect();

        if registry_names.is_empty() {
            println!("no registry-installed plugins to update");
            return Ok(());
        }

        for plugin_name in &registry_names {
            match update_one(plugin_name).await {
                Ok(_) => {}
                Err(e) => eprintln!("warning: failed to update '{}': {e}", plugin_name),
            }
        }
        return Ok(());
    }

    let name = name.ok_or_else(|| anyhow::anyhow!("provide a plugin name or use --all"))?;
    update_one(name).await
}

async fn update_one(name: &str) -> Result<()> {
    let update_available = tokio::task::spawn_blocking({
        let name = name.to_string();
        move || check_for_update(&name, true, None)
    })
    .await??;

    match update_available {
        None => println!("'{name}' is already up to date (or not registry-installed)"),
        Some((current, latest)) => {
            println!("updating '{name}': {current} → {latest}");
            tokio::task::spawn_blocking({
                let name = name.to_string();
                move || update_plugin(&name, None)
            })
            .await??;
            println!("updated '{name}' to {latest}");
        }
    }
    Ok(())
}

async fn cmd_exec(name: &str) -> Result<()> {
    // Locate the plugin in the plugins directory
    let lynx_dir = std::env::var("LYNX_DIR")
        .unwrap_or_else(|_| format!("{}/.local/share/lynx", std::env::var("HOME").unwrap_or_else(|_| ".".into())));
    let plugin_dir = PathBuf::from(&lynx_dir).join("plugins").join(name);

    // Also check repo-local plugins/ for development
    let repo_plugin_dir = PathBuf::from("plugins").join(name);
    let resolved_dir = if plugin_dir.exists() {
        plugin_dir
    } else if repo_plugin_dir.exists() {
        repo_plugin_dir
    } else {
        bail!("plugin '{}' not found. Run: lx doctor", name);
    };

    let manifest_path = resolved_dir.join("plugin.toml");
    if !manifest_path.exists() {
        bail!("plugin '{}' has no plugin.toml", name);
    }

    let content = std::fs::read_to_string(&manifest_path)?;
    let manifest = lynx_manifest::parse(&content).map_err(|e| anyhow::anyhow!("{}", e))?;

    let script = generate_exec_script(&manifest, &resolved_dir)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    print!("{}", script);
    Ok(())
}
