use lynx_core::error::{LynxError, Result};
use lynx_manifest::schema::PluginManifest;
use std::path::Path;

/// Generate the zsh activation script for a plugin.
///
/// This is what `lx plugin exec <name>` emits to stdout — the shell evals it.
/// Output sets `LYNX_PLUGIN_DIR` and sources the plugin's `shell/init.zsh`.
/// Uses `$LYNX_PLUGIN_DIR`-relative paths — never hardcoded absolutes.
pub fn generate_exec_script(manifest: &PluginManifest, plugin_dir: &Path) -> Result<String> {
    let init_zsh = plugin_dir.join("shell/init.zsh");
    if !init_zsh.exists() {
        return Err(LynxError::Plugin(format!(
            "plugin '{}' has no shell/init.zsh at {:?}",
            manifest.plugin.name, init_zsh
        )));
    }

    let dir_str = plugin_dir
        .to_str()
        .ok_or_else(|| LynxError::Plugin("plugin dir path is not valid UTF-8".into()))?;

    let mut out = String::new();

    // Idempotency guard keyed by plugin name
    let guard_var = format!(
        "LYNX_PLUGIN_{}_LOADED",
        manifest.plugin.name.to_uppercase().replace('-', "_")
    );

    out.push_str(&format!(
        "if [[ -z \"${{{}}}\" ]]; then\n",
        guard_var
    ));
    out.push_str(&format!(
        "  export LYNX_PLUGIN_DIR='{}'\n",
        dir_str.replace('\'', "'\\''")
    ));
    out.push_str(&format!(
        "  source \"$LYNX_PLUGIN_DIR/shell/init.zsh\" 2>/dev/null\n"
    ));
    for hook in &manifest.load.hooks {
        let fn_name = format!(
            "_{}_plugin_{}",
            manifest.plugin.name.replace('-', "_"),
            hook
        );
        out.push_str(&format!("  add-zsh-hook {} {}\n", hook, fn_name));
    }
    out.push_str(&format!("  export {}=1\n", guard_var));
    out.push_str("fi\n");

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lynx_manifest::schema::*;

    fn simple_manifest(name: &str) -> PluginManifest {
        PluginManifest {
            schema_version: 1,
            plugin: PluginMeta {
                name: name.into(),
                version: "0.1.0".into(),
                description: String::new(),
                authors: vec![],
            },
            load: LoadConfig::default(),
            deps: DepsConfig::default(),
            exports: ExportsConfig::default(),
            contexts: ContextsConfig::default(),
        }
    }

    #[test]
    fn exec_script_contains_required_elements() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("shell")).unwrap();
        std::fs::write(tmp.path().join("shell/init.zsh"), "# stub").unwrap();

        let m = simple_manifest("git");
        let script = generate_exec_script(&m, tmp.path()).unwrap();

        assert!(script.contains("LYNX_PLUGIN_DIR="));
        assert!(script.contains("source \"$LYNX_PLUGIN_DIR/shell/init.zsh\""));
        assert!(script.contains("LYNX_PLUGIN_GIT_LOADED"));
    }

    #[test]
    fn exec_script_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("shell")).unwrap();
        std::fs::write(tmp.path().join("shell/init.zsh"), "# stub").unwrap();

        let m = simple_manifest("git");
        let script = generate_exec_script(&m, tmp.path()).unwrap();
        assert!(script.contains("LYNX_PLUGIN_GIT_LOADED"));
    }

    #[test]
    fn exec_script_wires_hooks() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("shell")).unwrap();
        std::fs::write(tmp.path().join("shell/init.zsh"), "# stub").unwrap();

        let mut m = simple_manifest("git");
        m.load.hooks = vec!["chpwd".into(), "precmd".into()];
        let script = generate_exec_script(&m, tmp.path()).unwrap();

        assert!(script.contains("add-zsh-hook chpwd _git_plugin_chpwd"));
        assert!(script.contains("add-zsh-hook precmd _git_plugin_precmd"));
    }

    #[test]
    fn missing_init_zsh_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        let m = simple_manifest("broken");
        let err = generate_exec_script(&m, tmp.path()).unwrap_err();
        assert!(err.to_string().contains("no shell/init.zsh"));
    }
}
