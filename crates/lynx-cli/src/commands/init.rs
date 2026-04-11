use anyhow::Result;
use clap::Args;
use lynx_config::load as load_config;
use lynx_core::{brand, env_vars, types::Context};
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
            eprintln!(
                "lx: unknown context '{}', falling back to auto-detect",
                other
            );
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

    // Emit LS_COLORS / EZA_COLORS from the active theme so file listings are
    // colored from first shell startup without any manual eval.
    let theme_name = std::env::var(env_vars::LYNX_THEME)
        .unwrap_or_else(|_| config.active_theme.clone());
    if let Ok(theme) = load_theme(&theme_name).or_else(|_| load_theme(brand::DEFAULT_THEME)) {
        if let Some(ls) = theme.ls_colors.to_ls_colors_string() {
            print!("export LS_COLORS={ls:?}\n");
        }
        if let Some(eza) = theme.ls_colors.to_eza_colors_string() {
            print!("export EZA_COLORS={eza:?}\n");
        }
    }

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
