use lynx_manifest::schema::PluginManifest;

/// Generate zsh wrapper functions for all exports of a lazy plugin.
///
/// Each wrapper:
/// 1. Calls `lx plugin exec <name>` to activate the plugin on first use.
/// 2. Evals the output (LOAD + ACTIVATE).
/// 3. Re-invokes the original command so the user's call succeeds transparently.
/// 4. Undefines itself after activation (subsequent calls use the real function).
///
/// If the plugin takes >200ms, a brief loading message is shown.
pub fn generate_lazy_wrappers(manifest: &PluginManifest) -> String {
    let plugin_name = &manifest.plugin.name;
    let mut out = String::new();

    // Combine functions and aliases into a single wrapper set
    for fn_name in &manifest.exports.functions {
        out.push_str(&lazy_fn_wrapper(plugin_name, fn_name));
    }

    for alias_name in &manifest.exports.aliases {
        out.push_str(&lazy_alias_wrapper(plugin_name, alias_name));
    }

    out
}

fn lazy_fn_wrapper(plugin_name: &str, fn_name: &str) -> String {
    format!(
        r#"{fn_name}() {{
  local _t0=$EPOCHREALTIME
  local _out
  _out="$(lx plugin exec '{plugin_name}' 2>&1)"
  local _rc=$?
  if (( _rc != 0 )); then
    print -u2 "Lynx: lazy-load of '{plugin_name}' failed. Run: lx doctor"
    return $_rc
  fi
  eval "$_out"
  local _elapsed=$(( (EPOCHREALTIME - _t0) * 1000 ))
  (( _elapsed > 200 )) && print -u2 "[lynx] loaded plugin '{plugin_name}' (${{_elapsed%.*}}ms)"
  unfunction {fn_name} 2>/dev/null
  {fn_name} "$@"
}}
"#,
    )
}

fn lazy_alias_wrapper(plugin_name: &str, alias_name: &str) -> String {
    // Aliases are loaded via the plugin exec output — emit a function wrapper
    // that triggers the load then invokes the alias.
    format!(
        r#"{alias_name}() {{
  local _out
  _out="$(lx plugin exec '{plugin_name}' 2>&1)"
  local _rc=$?
  if (( _rc != 0 )); then
    print -u2 "Lynx: lazy-load of '{plugin_name}' failed. Run: lx doctor"
    return $_rc
  fi
  eval "$_out"
  unfunction {alias_name} 2>/dev/null
  {alias_name} "$@"
}}
"#,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use lynx_manifest::schema::*;

    fn lazy_manifest(fns: &[&str], aliases: &[&str]) -> PluginManifest {
        PluginManifest {
            schema_version: 1,
            plugin: PluginMeta {
                name: "git".into(),
                version: "0.1.0".into(),
                description: String::new(),
                authors: vec![],
            },
            load: LoadConfig {
                lazy: true,
                hooks: vec![],
            },
            deps: DepsConfig::default(),
            exports: ExportsConfig {
                functions: fns.iter().map(|s| s.to_string()).collect(),
                aliases: aliases.iter().map(|s| s.to_string()).collect(),
            },
            contexts: ContextsConfig::default(),
            state: StateConfig::default(),
            shell: ShellConfig::default(),
        }
    }

    #[test]
    fn wrappers_generated_for_all_exports() {
        let m = lazy_manifest(&["git_branch", "git_dirty"], &["gst", "gco"]);
        let zsh = generate_lazy_wrappers(&m);
        assert!(zsh.contains("git_branch()"));
        assert!(zsh.contains("git_dirty()"));
        assert!(zsh.contains("gst()"));
        assert!(zsh.contains("gco()"));
    }

    #[test]
    fn wrapper_calls_lx_plugin_exec() {
        let m = lazy_manifest(&["git_branch"], &[]);
        let zsh = generate_lazy_wrappers(&m);
        assert!(zsh.contains("lx plugin exec 'git'"));
    }

    #[test]
    fn wrapper_undefines_self_after_activation() {
        let m = lazy_manifest(&["git_branch"], &[]);
        let zsh = generate_lazy_wrappers(&m);
        assert!(zsh.contains("unfunction git_branch"));
    }

    #[test]
    fn empty_exports_produces_empty_output() {
        let m = lazy_manifest(&[], &[]);
        assert!(generate_lazy_wrappers(&m).is_empty());
    }
}
