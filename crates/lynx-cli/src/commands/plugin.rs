use anyhow::{bail, Result};
use clap::{Args, Subcommand};
use lynx_config::{load as load_config, save as save_config};
use lynx_plugin::exec::generate_exec_script;
use lynx_plugin::namespace::scaffold_convention_comment;
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
}

pub async fn run(args: PluginArgs) -> Result<()> {
    match args.command {
        PluginCommand::Add { path } => cmd_add(&path).await,
        PluginCommand::Remove { name } => cmd_remove(&name).await,
        PluginCommand::List { json } => cmd_list(json).await,
        PluginCommand::New { name } => cmd_new(&name).await,
        PluginCommand::Reinstall { name } => cmd_reinstall(&name).await,
        PluginCommand::Exec { name } => cmd_exec(&name).await,
        PluginCommand::Examples => {
            crate::commands::examples::run(
                crate::commands::examples::ExamplesArgs { command: Some("plugin".into()) }
            ).await
        }
    }
}

async fn cmd_add(path: &str) -> Result<()> {
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
    // For now: remove from config then prompt to re-add with a path.
    // Full registry fetch is a later block (lynx-registry).
    let mut config = load_config()?;
    config.enabled_plugins.retain(|p| p != name);
    save_config(&config)?;
    println!(
        "Removed '{}' from config. Re-add with: lx plugin add <path>",
        name
    );
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
