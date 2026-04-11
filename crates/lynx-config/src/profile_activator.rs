use std::collections::HashMap;

use lynx_core::redact::looks_like_secret_value;

use crate::profile::Profile;

/// Snapshot of the currently active state used to compute activation diffs.
#[derive(Debug, Clone, Default)]
pub struct ActiveState {
    /// Names of currently loaded plugins.
    pub plugins: Vec<String>,
    /// Currently active theme name.
    pub theme: Option<String>,
    /// Currently exported profile env vars (key → value).
    pub env: HashMap<String, String>,
    /// Currently loaded profile aliases (name → expansion).
    pub aliases: HashMap<String, String>,
}

/// Compute the incremental zsh needed to switch from `current` state to `profile`.
///
/// Returns a string of zsh statements ready to be eval'd by the shell.
/// Only changes what differs — no full session reload.
///
/// # Safety
/// Env values that look like secrets are silently dropped (not exported).
pub fn activate_profile(profile: &Profile, current: &ActiveState) -> String {
    let mut out = Vec::<String>::new();

    // ── Plugins ──────────────────────────────────────────────────────────────
    let to_unload: Vec<&String> = current
        .plugins
        .iter()
        .filter(|p| !profile.plugins.contains(p))
        .collect();
    let to_load: Vec<&String> = profile
        .plugins
        .iter()
        .filter(|p| !current.plugins.contains(p))
        .collect();

    for name in to_unload {
        out.push(format!("lx plugin unload {name} 2>/dev/null"));
    }
    for name in to_load {
        out.push(format!("eval \"$(lx plugin exec {name} 2>/dev/null)\""));
    }

    // ── Theme ─────────────────────────────────────────────────────────────────
    if let Some(ref new_theme) = profile.theme {
        if current.theme.as_deref() != Some(new_theme.as_str()) {
            out.push(format!(
                "eval \"$(lx theme apply {new_theme} 2>/dev/null)\""
            ));
        }
    }

    // ── Env vars ──────────────────────────────────────────────────────────────
    // Unset vars from the previous profile that are not in the new one.
    for key in current.env.keys() {
        if !profile.env.contains_key(key) {
            out.push(format!("unset {key}"));
        }
    }
    // Export new/changed vars, skipping secret-shaped keys.
    for (key, value) in &profile.env {
        if looks_like_secret_value(key, value) {
            // Silent drop — warned at parse time, not at activation time.
            continue;
        }
        let current_val = current.env.get(key);
        if current_val.map(|v| v.as_str()) != Some(value.as_str()) {
            out.push(format!("export {key}={value:?}"));
        }
    }

    // ── Aliases ───────────────────────────────────────────────────────────────
    // Remove aliases from previous profile not present in new one.
    for name in current.aliases.keys() {
        if !profile.aliases.contains_key(name) {
            out.push(format!("unalias {name} 2>/dev/null"));
        }
    }
    // Set new/changed aliases (context gate is in plugin loading — profiles
    // only apply aliases in interactive context via init.zsh logic).
    for (name, expansion) in &profile.aliases {
        let current_val = current.aliases.get(name);
        if current_val.map(|v| v.as_str()) != Some(expansion.as_str()) {
            out.push(format!("alias {name}={expansion:?}"));
        }
    }

    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Profile;

    fn profile(plugins: &[&str], theme: &str) -> Profile {
        Profile {
            name: "test".into(),
            extends: None,
            plugins: plugins.iter().map(|s| s.to_string()).collect(),
            theme: Some(theme.into()),
            context_override: None,
            env: HashMap::new(),
            aliases: HashMap::new(),
        }
    }

    fn state(plugins: &[&str], theme: &str) -> ActiveState {
        ActiveState {
            plugins: plugins.iter().map(|s| s.to_string()).collect(),
            theme: Some(theme.into()),
            env: HashMap::new(),
            aliases: HashMap::new(),
        }
    }

    #[test]
    fn load_new_plugin() {
        let p = profile(&["git", "kubectl"], "default");
        let s = state(&["git"], "default");
        let zsh = activate_profile(&p, &s);
        assert!(zsh.contains("lx plugin exec kubectl"));
        assert!(!zsh.contains("lx plugin unload"));
    }

    #[test]
    fn unload_removed_plugin() {
        let p = profile(&["git"], "default");
        let s = state(&["git", "kubectl"], "default");
        let zsh = activate_profile(&p, &s);
        assert!(zsh.contains("lx plugin unload kubectl"));
        assert!(!zsh.contains("lx plugin exec"));
    }

    #[test]
    fn no_changes_produces_empty_output() {
        let p = profile(&["git"], "default");
        let s = state(&["git"], "default");
        let zsh = activate_profile(&p, &s);
        assert!(zsh.is_empty());
    }

    #[test]
    fn theme_switch_emitted() {
        let p = profile(&[], "nord");
        let s = state(&[], "default");
        let zsh = activate_profile(&p, &s);
        assert!(zsh.contains("lx theme apply nord"));
    }

    #[test]
    fn same_theme_not_re_applied() {
        let p = profile(&[], "nord");
        let s = state(&[], "nord");
        let zsh = activate_profile(&p, &s);
        assert!(!zsh.contains("lx theme apply"));
    }

    #[test]
    fn env_var_exported() {
        let mut p = profile(&[], "default");
        p.env.insert("KUBECONFIG".into(), "~/.kube/work".into());
        let zsh = activate_profile(&p, &ActiveState::default());
        assert!(zsh.contains("export KUBECONFIG="));
    }

    #[test]
    fn secret_env_var_dropped() {
        let mut p = profile(&[], "default");
        p.env.insert("GITHUB_TOKEN".into(), "ghp_secret".into());
        let zsh = activate_profile(&p, &ActiveState::default());
        assert!(!zsh.contains("GITHUB_TOKEN"));
    }

    #[test]
    fn removed_env_var_unset() {
        let p = profile(&[], "default");
        let mut s = ActiveState::default();
        s.env.insert("FOO".into(), "bar".into());
        let zsh = activate_profile(&p, &s);
        assert!(zsh.contains("unset FOO"));
    }

    #[test]
    fn alias_set_and_removed() {
        let mut p = profile(&[], "default");
        p.aliases.insert("ll".into(), "ls -la".into());

        let mut s = ActiveState::default();
        s.aliases.insert("gs".into(), "git status".into());

        let zsh = activate_profile(&p, &s);
        assert!(zsh.contains("alias ll="));
        assert!(zsh.contains("unalias gs"));
    }

    #[test]
    fn output_is_valid_zsh_syntax() {
        // Basic check: none of the lines are obviously malformed.
        let mut p = profile(&["git", "kubectl"], "nord");
        p.env.insert("KUBECONFIG".into(), "~/.kube/work".into());
        p.aliases.insert("k".into(), "kubectl".into());

        let mut s = state(&["git", "fzf"], "default");
        s.env.insert("OLD_VAR".into(), "old".into());
        s.aliases.insert("old_alias".into(), "echo old".into());

        let zsh = activate_profile(&p, &s);
        // Every non-empty line should contain a zsh keyword we recognise.
        for line in zsh.lines().filter(|l| !l.is_empty()) {
            assert!(
                line.starts_with("eval ")
                    || line.starts_with("lx ")
                    || line.starts_with("export ")
                    || line.starts_with("unset ")
                    || line.starts_with("alias ")
                    || line.starts_with("unalias "),
                "unexpected zsh line: {line}"
            );
        }
    }
}
