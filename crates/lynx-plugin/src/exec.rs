use lynx_core::error::{LynxError, Result};
use lynx_manifest::schema::PluginManifest;
use std::path::Path;

/// Generate the zsh activation script for a plugin.
///
/// This is what `lx plugin exec <name>` emits to stdout — the shell evals it.
/// Output sets `LYNX_PLUGIN_DIR` and sources the plugin's `shell/init.zsh`.
/// Uses `$LYNX_PLUGIN_DIR`-relative paths — never hardcoded absolutes.
pub fn generate_exec_script(manifest: &PluginManifest, plugin_dir: &Path) -> Result<String> {
    validate_shell_fields(manifest)?;

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
    let guard_var = lynx_core::env_vars::plugin_guard_var(&manifest.plugin.name);

    // Binary dependency guards — emitted into the generated eval output (not static shell).
    // This is the authoritative location for binary checks; plugin shell/init.zsh must not
    // duplicate this logic (D-001: no logic in static shell files).
    for binary in &manifest.deps.binaries {
        out.push_str(&format!(
            "if ! command -v -- {} &>/dev/null; then\n",
            shell_single_quote(binary)
        ));
        out.push_str(&format!(
            "  echo \"lynx: plugin '{}' requires '{binary}' — install it first\" >&2\n",
            manifest.plugin.name
        ));
        out.push_str("  return 1\n");
        out.push_str("fi\n");
    }

    out.push_str(&format!("if [[ -z \"${{{guard_var}}}\" ]]; then\n"));
    // Not exported — LYNX_PLUGIN_DIR is shell-local state used only during plugin sourcing
    out.push_str(&format!(
        "  {}='{}'\n",
        lynx_core::env_vars::LYNX_PLUGIN_DIR,
        dir_str.replace('\'', "'\\''")
    ));
    // fpath prepends — must come before init.zsh so completions are available to compinit
    for fpath_dir in &manifest.shell.fpath {
        let escaped = fpath_dir.replace('\'', "'\\''");
        out.push_str(&format!(
            "  fpath=(\"${plugin_dir_var}/{escaped}\" $fpath)\n",
            plugin_dir_var = lynx_core::env_vars::LYNX_PLUGIN_DIR,
        ));
    }
    out.push_str(&format!(
        "  source \"${}/shell/init.zsh\" 2>/dev/null\n",
        lynx_core::env_vars::LYNX_PLUGIN_DIR
    ));
    for hook in &manifest.load.hooks {
        let hook_token = hook.replace('-', "_");
        let fn_name = format!(
            "_{}_plugin_{}",
            manifest.plugin.name.replace('-', "_"),
            hook_token
        );
        out.push_str(&format!("  add-zsh-hook {hook} {fn_name}\n"));
    }
    // ZLE widget registrations
    for widget in &manifest.shell.widgets {
        out.push_str(&format!("  zle -N {widget}\n"));
    }
    // Key bindings
    for kb in &manifest.shell.keybindings {
        out.push_str(&format!(
            "  bindkey '{}' {}\n",
            kb.key.replace('\'', "'\\''"),
            kb.widget
        ));
    }
    // Not exported — guard must not leak into child shells where aliases won't be inherited
    out.push_str(&format!("  typeset -g {guard_var}=1\n"));
    out.push_str("fi\n");

    Ok(out)
}

fn validate_shell_fields(manifest: &PluginManifest) -> Result<()> {
    ensure_plugin_name("plugin.name", &manifest.plugin.name)?;
    for bin in &manifest.deps.binaries {
        ensure_command_name("deps.binaries", bin)?;
    }
    for hook in &manifest.load.hooks {
        ensure_hook_name("load.hooks", hook)?;
    }
    for widget in &manifest.shell.widgets {
        ensure_widget_name("shell.widgets", widget)?;
    }
    for kb in &manifest.shell.keybindings {
        ensure_widget_name("shell.keybindings.widget", &kb.widget)?;
    }
    Ok(())
}

fn ensure_plugin_name(field: &str, value: &str) -> Result<()> {
    if !is_plugin_name(value) {
        return Err(LynxError::Manifest(format!(
            "invalid {field} value '{value}' — expected [A-Za-z0-9_-]+"
        )));
    }
    Ok(())
}

fn ensure_command_name(field: &str, value: &str) -> Result<()> {
    if !is_command_name(value) {
        return Err(LynxError::Manifest(format!(
            "invalid {field} value '{value}' — expected [A-Za-z0-9_.+-]+"
        )));
    }
    Ok(())
}

fn ensure_hook_name(field: &str, value: &str) -> Result<()> {
    if !is_hook_name(value) {
        return Err(LynxError::Manifest(format!(
            "invalid {field} value '{value}' — expected [A-Za-z0-9_]+"
        )));
    }
    Ok(())
}

fn ensure_widget_name(field: &str, value: &str) -> Result<()> {
    if !is_widget_name(value) {
        return Err(LynxError::Manifest(format!(
            "invalid {field} value '{value}' — expected [A-Za-z0-9_-]+"
        )));
    }
    Ok(())
}

fn is_plugin_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn is_command_name(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|ch| {
            ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.' || ch == '+'
        })
}

fn is_hook_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn is_widget_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
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
            state: StateConfig::default(),
            shell: ShellConfig::default(),
        }
    }

    #[test]
    fn exec_script_contains_required_elements() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("shell")).unwrap();
        std::fs::write(tmp.path().join("shell/init.zsh"), "# stub").unwrap();

        let m = simple_manifest("git");
        let script = generate_exec_script(&m, tmp.path()).unwrap();

        assert!(script.contains(&format!("{}=", lynx_core::env_vars::LYNX_PLUGIN_DIR)));
        assert!(script.contains(&format!(
            "source \"${}/shell/init.zsh\"",
            lynx_core::env_vars::LYNX_PLUGIN_DIR
        )));
        assert!(script.contains(&lynx_core::env_vars::plugin_guard_var("git")));
    }

    #[test]
    fn exec_script_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("shell")).unwrap();
        std::fs::write(tmp.path().join("shell/init.zsh"), "# stub").unwrap();

        let m = simple_manifest("git");
        let script = generate_exec_script(&m, tmp.path()).unwrap();
        assert!(script.contains(&lynx_core::env_vars::plugin_guard_var("git")));
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

    #[test]
    fn exec_script_emits_binary_guard_for_declared_deps() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("shell")).unwrap();
        std::fs::write(tmp.path().join("shell/init.zsh"), "# stub").unwrap();

        let mut m = simple_manifest("kubectl");
        m.deps.binaries = vec!["kubectl".into()];
        let script = generate_exec_script(&m, tmp.path()).unwrap();

        assert!(script.contains("command -v -- 'kubectl'"));
        assert!(script.contains("return 1"));
    }

    #[test]
    fn exec_script_no_binary_guard_when_no_deps() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("shell")).unwrap();
        std::fs::write(tmp.path().join("shell/init.zsh"), "# stub").unwrap();

        let m = simple_manifest("git");
        let script = generate_exec_script(&m, tmp.path()).unwrap();

        assert!(!script.contains("command -v"));
    }

    #[test]
    fn exec_script_emits_fpath_before_source() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("shell")).unwrap();
        std::fs::write(tmp.path().join("shell/init.zsh"), "# stub").unwrap();

        let mut m = simple_manifest("my-plugin");
        m.shell.fpath = vec!["completions".into()];
        let script = generate_exec_script(&m, tmp.path()).unwrap();

        assert!(script.contains(&format!(
            "fpath=(\"${}/completions\" $fpath)",
            lynx_core::env_vars::LYNX_PLUGIN_DIR
        )));
        // fpath must appear before source in the output
        let fpath_pos = script.find("fpath=(").unwrap();
        let source_pos = script
            .find(&format!(
                "source \"${}/shell/init.zsh\"",
                lynx_core::env_vars::LYNX_PLUGIN_DIR
            ))
            .unwrap();
        assert!(fpath_pos < source_pos, "fpath must come before source");
    }

    #[test]
    fn exec_script_emits_zle_widget_registration() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("shell")).unwrap();
        std::fs::write(tmp.path().join("shell/init.zsh"), "# stub").unwrap();

        let mut m = simple_manifest("fzf-plugin");
        m.shell.widgets = vec!["fzf_history_widget".into()];
        let script = generate_exec_script(&m, tmp.path()).unwrap();

        assert!(script.contains("zle -N fzf_history_widget"));
    }

    #[test]
    fn exec_script_emits_keybindings() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("shell")).unwrap();
        std::fs::write(tmp.path().join("shell/init.zsh"), "# stub").unwrap();

        let mut m = simple_manifest("fzf-plugin");
        m.shell.widgets = vec!["fzf_history_widget".into()];
        m.shell.keybindings = vec![KeyBinding {
            key: "^R".into(),
            widget: "fzf_history_widget".into(),
        }];
        let script = generate_exec_script(&m, tmp.path()).unwrap();

        assert!(script.contains("bindkey '^R' fzf_history_widget"));
        // zle -N must come before bindkey
        let zle_pos = script.find("zle -N").unwrap();
        let bindkey_pos = script.find("bindkey").unwrap();
        assert!(zle_pos < bindkey_pos, "zle -N must precede bindkey");
    }

    #[test]
    fn exec_script_no_fpath_when_not_declared() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("shell")).unwrap();
        std::fs::write(tmp.path().join("shell/init.zsh"), "# stub").unwrap();

        let m = simple_manifest("git");
        let script = generate_exec_script(&m, tmp.path()).unwrap();

        assert!(!script.contains("fpath=("));
        assert!(!script.contains("zle -N"));
        assert!(!script.contains("bindkey"));
    }

    #[test]
    fn exec_script_rejects_invalid_binary_identifier() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("shell")).unwrap();
        std::fs::write(tmp.path().join("shell/init.zsh"), "# stub").unwrap();

        let mut m = simple_manifest("git");
        m.deps.binaries = vec!["git;rm".into()];
        let err = generate_exec_script(&m, tmp.path()).unwrap_err();
        assert!(err.to_string().contains("deps.binaries"));
    }

    #[test]
    fn exec_script_rejects_invalid_hook_identifier() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("shell")).unwrap();
        std::fs::write(tmp.path().join("shell/init.zsh"), "# stub").unwrap();

        let mut m = simple_manifest("git");
        m.load.hooks = vec!["precmd$".into()];
        let err = generate_exec_script(&m, tmp.path()).unwrap_err();
        assert!(err.to_string().contains("load.hooks"));
    }

    #[test]
    fn exec_script_rejects_invalid_widget_identifier() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("shell")).unwrap();
        std::fs::write(tmp.path().join("shell/init.zsh"), "# stub").unwrap();

        let mut m = simple_manifest("git");
        m.shell.widgets = vec!["widget()".into()];
        let err = generate_exec_script(&m, tmp.path()).unwrap_err();
        assert!(err.to_string().contains("shell.widgets"));
    }

    #[test]
    fn exec_script_rejects_invalid_keybinding_widget_identifier() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("shell")).unwrap();
        std::fs::write(tmp.path().join("shell/init.zsh"), "# stub").unwrap();

        let mut m = simple_manifest("git");
        m.shell.keybindings = vec![KeyBinding {
            key: "^R".into(),
            widget: "bad widget".into(),
        }];
        let err = generate_exec_script(&m, tmp.path()).unwrap_err();
        assert!(err.to_string().contains("shell.keybindings.widget"));
    }

    #[test]
    fn exec_script_rejects_invalid_plugin_name() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("shell")).unwrap();
        std::fs::write(tmp.path().join("shell/init.zsh"), "# stub").unwrap();

        let m = simple_manifest("bad$name");
        let err = generate_exec_script(&m, tmp.path()).unwrap_err();
        assert!(err.to_string().contains("plugin.name"));
    }
}
