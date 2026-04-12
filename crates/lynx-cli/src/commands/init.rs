use anyhow::Result;
use clap::Args;
use lynx_config::load as load_config;
use lynx_core::{brand, diag, env_vars, types::Context};
use lynx_theme::loader::load as load_theme;
use lynx_manifest::schema::PluginManifest;
use lynx_shell::{
    context::detect_context,
    init::{generate_init_script, InitParams},
    safemode::generate_safemode_script,
};

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
            diag::error("init", &format!("config load failed: {e}"));
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
            let msg = format!("unknown context '{}', falling back to auto-detect", other);
            diag::warn("init", &msg);
            detected
        }
        None => detected,
    };

    let lynx_dir = lynx_core::paths::lynx_dir().to_string_lossy().into_owned();
    let plugin_dir = lynx_core::paths::installed_plugins_dir()
        .to_string_lossy()
        .into_owned();

    // Resolve load order: topological sort, binary dep exclusion, lazy/eager split.
    let manifests = load_plugin_manifests(&plugin_dir, &config.enabled_plugins);
    let enabled_plugins: Vec<String> = match lynx_depgraph::depgraph::resolve(&manifests) {
        Ok(order) => {
            for (name, bin) in &order.excluded {
                let msg = format!("plugin '{}' excluded — missing binary '{}'", name, bin);
                diag::warn("init", &msg);
            }
            order.eager.into_iter().chain(order.lazy).collect()
        }
        Err(e) => {
            diag::error("init", &format!("plugin dependency error: {e}"));
            config.enabled_plugins.clone()
        }
    };

    // Load theme colors to embed inside the init script (inside the LYNX_INITIALIZED guard).
    let theme_name = std::env::var(env_vars::LYNX_THEME)
        .unwrap_or_else(|_| config.active_theme.clone());
    let theme_result = load_theme(&theme_name).or_else(|_| load_theme(brand::DEFAULT_THEME));
    let (ls_colors_str, eza_colors_str) = match &theme_result {
        Ok(theme) => (
            theme.ls_colors.to_ls_colors_string(),
            theme.ls_colors.to_eza_colors_string(),
        ),
        Err(e) => {
            diag::warn("init", &format!("theme '{}' failed to load: {e}", theme_name));
            (None, None)
        }
    };

    let script = generate_init_script(&InitParams {
        context: &context,
        lynx_dir: &lynx_dir,
        plugin_dir: &plugin_dir,
        enabled_plugins: &enabled_plugins,
        ls_colors: ls_colors_str.as_deref(),
        eza_colors: eza_colors_str.as_deref(),
    });

    print!("{}", script);
    Ok(())
}

/// Load plugin.toml manifests for the given plugin names from plugin_dir.
/// Plugins with missing or invalid manifests are skipped; errors go to the diag log.
fn load_plugin_manifests(plugin_dir: &str, enabled: &[String]) -> Vec<PluginManifest> {
    let mut manifests = Vec::new();
    for name in enabled {
        let toml_path = format!("{}/{}/plugin.toml", plugin_dir, name);
        if let Ok(content) = std::fs::read_to_string(&toml_path) {
            match lynx_manifest::parse_and_validate(&content) {
                Ok(m) => manifests.push(m),
                Err(e) => diag::warn("init", &format!("skipping plugin '{}': invalid manifest: {}", name, e)),
            }
        } // plugin dir missing or no manifest — silently skip (expected for optional plugins)
    }
    manifests
}
