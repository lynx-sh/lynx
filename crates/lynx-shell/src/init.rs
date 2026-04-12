use std::collections::HashSet;

use lynx_core::env_vars;
use lynx_core::types::Context;

/// Parameters required to generate the shell init script.
pub struct InitParams<'a> {
    pub context: &'a Context,
    pub lynx_dir: &'a str,
    pub plugin_dir: &'a str,
    pub enabled_plugins: &'a [String],
    /// LS_COLORS value from the active theme. Emitted inside the init guard.
    pub ls_colors: Option<&'a str>,
    /// EZA_COLORS value from the active theme. Emitted inside the init guard.
    pub eza_colors: Option<&'a str>,
    /// BSD LSCOLORS value for macOS /bin/ls.
    pub bsd_lscolors: Option<&'a str>,
    /// ZSH_HIGHLIGHT_STYLES assignments from the active theme.
    pub syntax_highlight_styles: Option<&'a str>,
    /// ZSH_AUTOSUGGEST_HIGHLIGHT_STYLE value from the active theme.
    pub autosuggest_style: Option<&'a str>,
    /// Plugins that hook into ZLE (zle -N) and must be sourced directly.
    /// These cannot go through eval "$()" — zle widget binding fails inside eval.
    pub zle_hook_plugins: HashSet<String>,
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

    // File-listing colors from the active theme (LS_COLORS / EZA_COLORS).
    // Emitted inside the init guard so they are set exactly once per session.
    if let Some(ls) = params.ls_colors {
        out.push_str(&format!("  export LS_COLORS={}\n", shell_quote(ls)));
    }
    if let Some(eza) = params.eza_colors {
        out.push_str(&format!("  export EZA_COLORS={}\n", shell_quote(eza)));
    }
    // BSD ls colors (macOS) — CLICOLOR enables color, LSCOLORS sets the palette.
    if let Some(bsd) = params.bsd_lscolors {
        out.push_str("  export CLICOLOR=1\n");
        out.push_str(&format!("  export LSCOLORS={}\n", shell_quote(bsd)));
    }

    // Smart ls alias — macOS /bin/ls can't use LS_COLORS (truecolor, per-extension).
    // Alias to eza or gls if available so theme colors actually render.
    if params.ls_colors.is_some() {
        out.push_str(concat!(
            "  if (( ! ${+aliases[ls]} )); then\n",
            "    if (( $+commands[eza] )); then\n",
            "      alias ls='eza'\n",
            "      alias ll='eza -la'\n",
            "      alias la='eza -a'\n",
            "      alias lt='eza --tree'\n",
            "    elif (( $+commands[gls] )); then\n",
            "      alias ls='gls --color=auto'\n",
            "      alias ll='gls --color=auto -la'\n",
            "      alias la='gls --color=auto -a'\n",
            "    fi\n",
            "  fi\n",
        ));
    }

    // Syntax highlighting styles from the active theme.
    // Emitted as individual ZSH_HIGHLIGHT_STYLES assignments that plugins source.
    if let Some(styles) = params.syntax_highlight_styles {
        // Declare the associative array first (idempotent — typeset -gA is safe to repeat).
        out.push_str("  typeset -gA ZSH_HIGHLIGHT_STYLES 2>/dev/null\n");
        for line in styles.lines() {
            out.push_str(&format!("  {line}\n"));
        }
    }
    // Auto-suggestion highlight style from the active theme.
    if let Some(style) = params.autosuggest_style {
        out.push_str(&format!(
            "  export ZSH_AUTOSUGGEST_HIGHLIGHT_STYLE={}\n",
            shell_quote(style)
        ));
    }

    // HOSTNAME: macOS zsh special param — not exported by default.
    out.push_str("  export HOSTNAME=\"${HOSTNAME:-$(hostname -s)}\"\n");

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

    // Source command dispatch — defines lx() wrapper that evals output for
    // subcommands like `theme set` and `context set` that emit shell assignments.
    out.push_str(&format!(
        "  source {dir}/shell/lib/commands.zsh 2>/dev/null\n",
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

    // Load each enabled plugin.
    // ZLE-hook plugins (syntax-highlight, autosuggestions, etc.) must be sourced
    // directly — zle -N widget binding fails inside eval "$(...)".
    // All other plugins go through lynx_eval_plugin (the standard eval-bridge).
    for plugin in params.enabled_plugins {
        if params.zle_hook_plugins.contains(plugin.as_str()) {
            let guard = env_vars::plugin_guard_var(plugin);
            let full_plugin_dir = shell_quote(&format!("{}/{}", params.plugin_dir, plugin));
            out.push_str(&format!(
                "  if [[ -z \"${{{guard}}}\" ]]; then\n    LYNX_PLUGIN_DIR={full_plugin_dir}\n    source \"$LYNX_PLUGIN_DIR/shell/init.zsh\" 2>/dev/null\n    typeset -g {guard}=1\n  fi\n",
                guard = guard,
                full_plugin_dir = full_plugin_dir,
            ));
        } else {
            out.push_str(&format!("  lynx_eval_plugin {}\n", shell_quote(plugin)));
        }
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
            ls_colors: None,
            eza_colors: None,
            bsd_lscolors: None,
            syntax_highlight_styles: None,
            autosuggest_style: None,
            zle_hook_plugins: HashSet::new(),
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
    fn zle_hook_plugin_emits_direct_source_not_eval_bridge() {
        let plugins = vec!["syntax-highlight".to_string(), "git".to_string()];
        let mut params = base_params(&Context::Interactive, &plugins);
        params.zle_hook_plugins = HashSet::from(["syntax-highlight".to_string()]);
        let script = generate_init_script(&params);
        // ZLE plugin must use direct source, not lynx_eval_plugin
        assert!(!script.contains("lynx_eval_plugin 'syntax-highlight'"));
        assert!(script.contains("source \"$LYNX_PLUGIN_DIR/shell/init.zsh\""));
        // Non-ZLE plugin still uses eval bridge
        assert!(script.contains("lynx_eval_plugin 'git'"));
    }

    #[test]
    fn no_plugins_produces_valid_structure() {
        let script = generate_init_script(&base_params(&Context::Minimal, &[]));
        assert!(script.contains("LYNX_CONTEXT=minimal"));
        assert!(!script.contains("lynx_eval_plugin"));
    }

    #[test]
    fn ls_colors_emitted_inside_guard_when_provided() {
        let plugins = vec![];
        let mut params = base_params(&Context::Interactive, &plugins);
        params.ls_colors = Some("di=1;34");
        params.eza_colors = Some("di=1;34");
        let script = generate_init_script(&params);
        // Both must be inside the if-guard, not before it
        let guard_pos = script.find("if [[").unwrap();
        let ls_pos = script.find("LS_COLORS='di=1;34'").unwrap();
        let eza_pos = script.find("EZA_COLORS='di=1;34'").unwrap();
        assert!(ls_pos > guard_pos, "LS_COLORS must be inside the init guard");
        assert!(eza_pos > guard_pos, "EZA_COLORS must be inside the init guard");
    }

    #[test]
    fn ls_colors_absent_when_not_provided() {
        let script = generate_init_script(&base_params(&Context::Interactive, &[]));
        assert!(!script.contains("LS_COLORS="), "LS_COLORS must not appear when not provided");
    }

    #[test]
    fn hostname_inside_guard() {
        let script = generate_init_script(&base_params(&Context::Interactive, &[]));
        let guard_pos = script.find("if [[").unwrap();
        let hostname_pos = script.find("HOSTNAME").unwrap();
        assert!(hostname_pos > guard_pos, "HOSTNAME must be inside the init guard");
    }
}
