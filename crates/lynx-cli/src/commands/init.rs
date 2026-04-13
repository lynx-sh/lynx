use anyhow::Result;
use clap::{Args, CommandFactory};
use clap_complete::{generate, Shell};
use lynx_config::load as load_config;
use lynx_core::{brand, diag, env_vars, types::Context};
use lynx_manifest::schema::PluginManifest;
use lynx_shell::{
    context::detect_context,
    init::{generate_init_script, InitParams},
    safemode::generate_safemode_script,
};
use lynx_theme::loader::load as load_theme;

/// Cooldown timestamp filename in the runtime dir.
const INTRO_LAST_SHOWN_FILE: &str = "intro_last_shown";

#[derive(Args)]
pub struct InitArgs {
    /// Override the detected context (interactive | agent | minimal)
    #[arg(long)]
    pub context: Option<String>,
}

pub fn run(args: InitArgs) -> Result<()> {
    // If config fails to load, emit safe mode instead of crashing the shell.
    let config = match load_config() {
        Ok(cfg) => cfg,
        Err(e) => {
            diag::error("init", &format!("config load failed: {e}"));
            let script = generate_safemode_script(&e.to_string());
            print!("{script}");
            return Ok(());
        }
    };

    let detected = detect_context();
    let context = match args.context.as_deref() {
        Some("agent") => Context::Agent,
        Some("minimal") => Context::Minimal,
        Some("interactive") => Context::Interactive,
        Some(other) => {
            let msg = format!("unknown context '{other}', falling back to auto-detect");
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
                let msg = format!("plugin '{name}' excluded — missing binary '{bin}'");
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
    let theme_name =
        std::env::var(env_vars::LYNX_THEME).unwrap_or_else(|_| config.active_theme.clone());
    let theme_result = load_theme(&theme_name).or_else(|_| load_theme(brand::DEFAULT_THEME));
    let (ls_colors_str, eza_colors_str, bsd_lscolors_str, syntax_styles_str, autosuggest_str) =
        match &theme_result {
            Ok(theme) => (
                theme.ls_colors.to_ls_colors_string(),
                theme.ls_colors.to_eza_colors_string(),
                Some(theme.ls_colors.to_bsd_lscolors()),
                theme.syntax_highlight.to_zsh_highlight_styles(),
                theme.autosuggestions.to_autosuggest_style(),
            ),
            Err(e) => {
                diag::warn("init", &format!("theme '{theme_name}' failed to load: {e}"));
                (None, None, None, None, None)
            }
        };

    // Display intro if enabled and in interactive context.
    // Must print BEFORE the eval script so it appears above the first prompt.
    maybe_show_intro(&config, context.clone());

    // Collect plugins that require direct source (zle_hook = true in plugin.toml).
    // These cannot go through eval "$()" — zle -N widget binding fails inside eval.
    let zle_hook_plugins: std::collections::HashSet<String> = manifests
        .iter()
        .filter(|m| m.shell.zle_hook)
        .map(|m| m.plugin.name.clone())
        .collect();

    let user_paths: Vec<String> = config.paths.iter().map(|p| p.path.clone()).collect();

    // Write _lx completion function to $LYNX_DIR/shell/completions/_lx.
    // Adding that dir to $fpath lets compinit pick it up regardless of whether
    // compinit has already run. We also conditionally call compdef if it's available.
    let completions_dir = lynx_core::paths::lynx_dir().join("shell").join("completions");
    let _ = std::fs::create_dir_all(&completions_dir);
    let completions_file = completions_dir.join("_lx");
    let completions_zsh = {
        let mut buf = Vec::new();
        let mut cmd = crate::cli::Cli::command();
        generate(Shell::Zsh, &mut cmd, "lx", &mut buf);
        String::from_utf8(buf).unwrap_or_default()
    };
    // Only write if changed — avoids touching mtime on every shell start.
    let should_write = std::fs::read_to_string(&completions_file)
        .map(|existing| existing != completions_zsh)
        .unwrap_or(true);
    if should_write {
        let _ = std::fs::write(&completions_file, &completions_zsh);
    }
    let completions_dir_str = completions_dir.to_string_lossy().into_owned();

    let script = generate_init_script(&InitParams {
        context: &context,
        lynx_dir: &lynx_dir,
        plugin_dir: &plugin_dir,
        enabled_plugins: &enabled_plugins,
        ls_colors: ls_colors_str.as_deref(),
        eza_colors: eza_colors_str.as_deref(),
        bsd_lscolors: bsd_lscolors_str.as_deref(),
        syntax_highlight_styles: syntax_styles_str.as_deref(),
        autosuggest_style: autosuggest_str.as_deref(),
        zle_hook_plugins,
        user_aliases: &config.aliases,
        user_paths: &user_paths,
        editor: config.editor.as_deref(),
        completions_zsh: Some(&completions_dir_str),
    });

    print!("{script}");
    Ok(())
}

/// Display the active intro if intro is enabled and we're in interactive context.
/// All errors are caught and logged — this must never crash the shell init.
fn maybe_show_intro(config: &lynx_config::schema::LynxConfig, context: Context) {
    if !config.intro.enabled || context != Context::Interactive {
        return;
    }
    let slug = match config.intro.active.as_deref() {
        Some(s) => s,
        None => return, // enabled but no intro selected — silent no-op
    };

    // Check cooldown.
    if let Ok(intro) = lynx_intro::loader::load(slug) {
        let cooldown = intro.display.cooldown_sec;
        if cooldown > 0 {
            if let Ok(rt_dir) = lynx_core::runtime::runtime_dir() {
                let stamp_path = rt_dir.join(INTRO_LAST_SHOWN_FILE);
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                if let Ok(content) = std::fs::read_to_string(&stamp_path) {
                    if let Ok(last) = content.trim().parse::<u64>() {
                        if now.saturating_sub(last) < cooldown {
                            return; // within cooldown window — skip
                        }
                    }
                }
                // Update timestamp (best-effort).
                std::fs::write(&stamp_path, now.to_string()).ok();
            }
        }

        let env: std::collections::HashMap<String, String> = std::env::vars().collect();
        let tokens = lynx_intro::build_token_map(&env);
        let rendered = lynx_intro::render_intro(&intro, &tokens);
        // MUST use eprint! — lx init output is eval'd by the shell.
        // Stdout gets executed as zsh; stderr goes directly to the terminal.
        eprint!("{rendered}");
    } else {
        diag::warn("init", &format!("intro '{slug}' failed to load — skipping"));
    }
}

/// Load plugin.toml manifests for the given plugin names from plugin_dir.
/// Plugins with missing or invalid manifests are skipped; errors go to the diag log.
fn load_plugin_manifests(plugin_dir: &str, enabled: &[String]) -> Vec<PluginManifest> {
    let mut manifests = Vec::new();
    for name in enabled {
        let toml_path = format!("{plugin_dir}/{name}/plugin.toml");
        if let Ok(content) = std::fs::read_to_string(&toml_path) {
            match lynx_manifest::parse_and_validate(&content) {
                Ok(m) => manifests.push(m),
                Err(e) => diag::warn(
                    "init",
                    &format!("skipping plugin '{name}': invalid manifest: {e}"),
                ),
            }
        } // plugin dir missing or no manifest — silently skip (expected for optional plugins)
    }
    manifests
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_plugin_manifests_empty_list() {
        let manifests = load_plugin_manifests("/nonexistent", &[]);
        assert!(manifests.is_empty());
    }

    #[test]
    fn load_plugin_manifests_missing_dir_returns_empty() {
        let manifests = load_plugin_manifests("/nonexistent/dir", &["git".to_string()]);
        assert!(manifests.is_empty());
    }

    #[test]
    fn load_plugin_manifests_invalid_toml_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let plugin_dir = tmp.path().join("test-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(plugin_dir.join("plugin.toml"), "not valid toml {{{{").unwrap();

        let manifests =
            load_plugin_manifests(tmp.path().to_str().unwrap(), &["test-plugin".to_string()]);
        assert!(manifests.is_empty());
    }

    #[test]
    fn load_plugin_manifests_valid_manifest_loaded() {
        let tmp = tempfile::tempdir().unwrap();
        let plugin_dir = tmp.path().join("git");
        std::fs::create_dir_all(&plugin_dir).unwrap();

        let manifest = r#"
[plugin]
name = "git"
version = "1.0.0"
description = "Git integration"

[load]
lazy = false
hooks = []

[deps]
binaries = ["git"]
plugins = []

[exports]
functions = ["git_status"]
aliases = []

[contexts]
disabled_in = ["agent"]
"#;
        std::fs::write(plugin_dir.join("plugin.toml"), manifest).unwrap();

        let manifests = load_plugin_manifests(tmp.path().to_str().unwrap(), &["git".to_string()]);
        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].plugin.name, "git");
    }

    #[test]
    fn maybe_show_intro_disabled_is_noop() {
        let mut cfg = lynx_config::schema::LynxConfig::default();
        cfg.intro.enabled = false;
        // Should return immediately without panic
        maybe_show_intro(&cfg, Context::Interactive);
    }

    #[test]
    fn maybe_show_intro_non_interactive_is_noop() {
        let mut cfg = lynx_config::schema::LynxConfig::default();
        cfg.intro.enabled = true;
        cfg.intro.active = Some("default".into());
        // Agent context should skip intro
        maybe_show_intro(&cfg, Context::Agent);
    }

    #[test]
    fn maybe_show_intro_no_active_slug_is_noop() {
        let mut cfg = lynx_config::schema::LynxConfig::default();
        cfg.intro.enabled = true;
        cfg.intro.active = None;
        maybe_show_intro(&cfg, Context::Interactive);
    }

    #[test]
    fn init_args_default_no_context_override() {
        use clap::Parser;
        #[derive(Parser)]
        struct W {
            #[command(flatten)]
            args: InitArgs,
        }
        let w = W::parse_from(["test"]);
        assert!(w.args.context.is_none());
    }

    #[test]
    fn init_args_context_override() {
        use clap::Parser;
        #[derive(Parser)]
        struct W {
            #[command(flatten)]
            args: InitArgs,
        }
        let w = W::parse_from(["test", "--context", "agent"]);
        assert_eq!(w.args.context.as_deref(), Some("agent"));
    }
}
