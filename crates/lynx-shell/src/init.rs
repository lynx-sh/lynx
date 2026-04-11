use lynx_core::env_vars;
use lynx_core::types::Context;

/// Parameters required to generate the shell init script.
pub struct InitParams<'a> {
    pub context: &'a Context,
    pub lynx_dir: &'a str,
    pub plugin_dir: &'a str,
    pub enabled_plugins: &'a [String],
}

/// Generate the zsh init script that the shell evals on startup.
///
/// Output is deterministic and side-effect-free. All logic is in Rust;
/// the zsh side does nothing but eval this string (D-001).
pub fn generate_init_script(params: &InitParams<'_>) -> String {
    let mut out = String::new();

    // Clear any inherited LYNX_INITIALIZED from parent shells so this shell always
    // initializes fresh. The idempotency guard below then prevents double-init if
    // this script is eval'd twice in the same session (e.g. double source of .zshrc).
    out.push_str(&format!("unset {}\n", env_vars::LYNX_INITIALIZED));

    // Guard: skip if already initialized (idempotency within same session)
    out.push_str(&format!(
        "if [[ -z \"${{{}}}\" ]]; then\n",
        env_vars::LYNX_INITIALIZED
    ));

    // Core env vars
    out.push_str(&format!(
        "  export {lynx_dir}={dir}\n  export {ctx_var}={ctx}\n  export {plugin_dir_var}={pdir}\n",
        lynx_dir = env_vars::LYNX_DIR,
        dir = shell_quote(params.lynx_dir),
        ctx_var = env_vars::LYNX_CONTEXT,
        ctx = params.context.as_str(),
        plugin_dir_var = env_vars::LYNX_PLUGIN_DIR,
        pdir = shell_quote(params.plugin_dir),
    ));

    // Source hook bridge (registered once per session)
    out.push_str(&format!(
        "  source {dir}/shell/core/hooks.zsh 2>/dev/null\n",
        dir = shell_quote(params.lynx_dir),
    ));

    // Source eval-bridge so lynx_eval_plugin / lynx_eval_safe are available
    out.push_str(&format!(
        "  source {dir}/shell/lib/eval-bridge.zsh 2>/dev/null\n",
        dir = shell_quote(params.lynx_dir),
    ));

    // Clear any inherited plugin load guards — guards must be shell-session-local.
    // A parent shell may have exported LYNX_PLUGIN_*_LOADED; if inherited, the
    // guard would block loading while aliases (shell-local) are not present.
    for plugin in params.enabled_plugins {
        out.push_str(&format!(
            "  unset {}\n",
            env_vars::plugin_guard_var(plugin)
        ));
    }

    // Eval-bridge calls for each enabled plugin
    for plugin in params.enabled_plugins {
        out.push_str(&format!("  lynx_eval_plugin {}\n", shell_quote(plugin)));
    }

    // Not exported — must not leak into child shells or lx init would skip there too
    out.push_str(&format!(
        "  typeset -g {}=1\n",
        env_vars::LYNX_INITIALIZED
    ));
    out.push_str("fi\n");

    out
}

/// Minimal shell quoting: wraps value in single-quotes, escaping internal single-quotes.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_params<'a>(ctx: &'a Context, plugins: &'a [String]) -> InitParams<'a> {
        InitParams {
            context: ctx,
            lynx_dir: "/home/user/.local/share/lynx",
            plugin_dir: "/home/user/.local/share/lynx/plugins",
            enabled_plugins: plugins,
        }
    }

    #[test]
    fn output_contains_required_exports() {
        let plugins = vec!["git".to_string()];
        let script = generate_init_script(&base_params(&Context::Interactive, &plugins));
        assert!(script.contains("LYNX_DIR="));
        assert!(script.contains("LYNX_CONTEXT=interactive"));
        assert!(script.contains("LYNX_PLUGIN_DIR="));
    }

    #[test]
    fn eval_bridge_is_sourced_before_plugins() {
        let plugins = vec!["git".to_string()];
        let script = generate_init_script(&base_params(&Context::Interactive, &plugins));
        assert!(
            script.contains("eval-bridge.zsh"),
            "eval-bridge must be sourced"
        );
        // eval-bridge must appear before the first lynx_eval_plugin call
        let bridge_pos = script.find("eval-bridge.zsh").unwrap();
        let plugin_pos = script.find("lynx_eval_plugin").unwrap();
        assert!(
            bridge_pos < plugin_pos,
            "eval-bridge must be sourced before plugin calls"
        );
    }

    #[test]
    fn daemon_is_not_autostarted_on_init() {
        let script = generate_init_script(&base_params(&Context::Interactive, &[]));
        assert!(
            !script.contains("lx daemon"),
            "daemon must not auto-start on shell init — it is opt-in only"
        );
    }

    #[test]
    fn agent_context_sets_correct_value() {
        let script = generate_init_script(&base_params(&Context::Agent, &[]));
        assert!(script.contains("LYNX_CONTEXT=agent"));
    }

    #[test]
    fn enabled_plugins_emit_eval_bridge_calls() {
        let plugins = vec!["git".to_string(), "fzf".to_string()];
        let script = generate_init_script(&base_params(&Context::Interactive, &plugins));
        assert!(script.contains("lynx_eval_plugin 'git'"));
        assert!(script.contains("lynx_eval_plugin 'fzf'"));
    }

    #[test]
    fn idempotency_guard_present() {
        let script = generate_init_script(&base_params(&Context::Interactive, &[]));
        assert!(script.contains("LYNX_INITIALIZED"));
    }

    #[test]
    fn no_plugins_produces_valid_structure() {
        let script = generate_init_script(&base_params(&Context::Minimal, &[]));
        assert!(script.contains("LYNX_CONTEXT=minimal"));
        assert!(!script.contains("lynx_eval_plugin"));
    }
}
