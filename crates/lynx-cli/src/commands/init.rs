use anyhow::Result;
use clap::Args;
use lynx_config::load as load_config;
use lynx_manifest::schema::PluginManifest;
use lynx_shell::{
    context::detect_context,
    init::{generate_init_script, InitParams},
    safemode::generate_safemode_script,
};
use lynx_core::types::Context;

#[derive(Args)]
pub struct InitArgs {
    /// Override the detected context (interactive | agent | minimal)
    #[arg(long)]
    pub context: Option<String>,
}

pub async fn run(args: InitArgs) -> Result<()> {
    // If config fails to load, emit safe mode instead of crashing the shell.
    let config = match load_config() {
        Ok(cfg) => cfg,
        Err(e) => {
            let script = generate_safemode_script(&e.to_string());
            print!("{}", script);
            return Ok(());
        }
    };

    let detected = detect_context();
    let context = match args.context.as_deref() {
        Some("agent") => Context::Agent,
        Some("minimal") => Context::Minimal,
        Some("interactive") => Context::Interactive,
        Some(other) => {
            eprintln!("lx: unknown context '{}', falling back to auto-detect", other);
            detected
        }
        None => detected,
    };

    let lynx_dir = resolve_lynx_dir();
    let plugin_dir = format!("{}/plugins", lynx_dir);

    // Resolve load order: topological sort, binary dep exclusion, lazy/eager split.
    let manifests = load_plugin_manifests(&plugin_dir, &config.enabled_plugins);
    let enabled_plugins: Vec<String> = match lynx_loader::depgraph::resolve(&manifests) {
        Ok(order) => {
            for (name, bin) in &order.excluded {
                eprintln!("lx: plugin '{}' excluded — missing binary '{}'", name, bin);
            }
            order.eager.into_iter().chain(order.lazy).collect()
        }
        Err(e) => {
            eprintln!("lx: plugin dependency error: {}", e);
            config.enabled_plugins.clone()
        }
    };

    let script = generate_init_script(&InitParams {
        context: &context,
        lynx_dir: &lynx_dir,
        plugin_dir: &plugin_dir,
        enabled_plugins: &enabled_plugins,
    });

    print!("{}", script);
    Ok(())
}

/// Load plugin.toml manifests for the given plugin names from plugin_dir.
/// Plugins with missing or invalid manifests are skipped with a diagnostic.
fn load_plugin_manifests(plugin_dir: &str, enabled: &[String]) -> Vec<PluginManifest> {
    let mut manifests = Vec::new();
    for name in enabled {
        let toml_path = format!("{}/{}/plugin.toml", plugin_dir, name);
        if let Ok(content) = std::fs::read_to_string(&toml_path) {
            match lynx_manifest::parse_and_validate(&content) {
                Ok(m) => manifests.push(m),
                Err(e) => eprintln!("lx: skipping plugin '{}': invalid manifest: {}", name, e),
            }
        } // plugin dir missing or no manifest — silently skip
    }
    manifests
}

/// Resolve LYNX_DIR: env override → default install location.
fn resolve_lynx_dir() -> String {
    if let Ok(dir) = std::env::var("LYNX_DIR") {
        return dir;
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    format!("{}/.local/share/lynx", home)
}
