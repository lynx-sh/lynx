use std::collections::HashSet;

use lynx_config::schema::{AliasContext, UserAlias};
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
    /// User-defined aliases to emit in the init script (context-gated).
    /// Only emitted when context is Interactive.
    pub user_aliases: &'a [UserAlias],
    /// User-defined PATH entries to prepend. Always emitted regardless of context.
    pub user_paths: &'a [String],
    /// Preferred editor binary from config (e.g. `code`, `zed`, `vim`).
    /// When set, exported as `$VISUAL` only if `$VISUAL` is not already set in the environment.
    pub editor: Option<&'a str>,
    /// Path to the directory containing the `_lx` completion file.
    /// Added to `$fpath` so compinit finds it, plus conditional `compdef` for
    /// shells where compinit has already run (e.g. macOS /etc/zshrc).
    pub completions_zsh: Option<&'a str>,
    /// When true, emit collision-guarded shell function wrappers for a curated set
    /// of safe lx subcommands (theme, plugin, doctor, …) so users can omit `lx`.
    /// Only honored in interactive context. Default: false.
    pub bare_subcommands: bool,
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
        out.push_str(&format!("  unset {}\n", env_vars::plugin_guard_var(plugin)));
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
                "  if [[ -z \"${{{guard}}}\" ]]; then\n    {plugin_dir_var}={full_plugin_dir}\n    source \"${plugin_dir_ref}/shell/init.zsh\" 2>/dev/null\n    typeset -g {guard}=1\n  fi\n",
                plugin_dir_var = env_vars::LYNX_PLUGIN_DIR,
                plugin_dir_ref = env_vars::LYNX_PLUGIN_DIR,
            ));
        } else {
            out.push_str(&format!("  lynx_eval_plugin {}\n", shell_quote(plugin)));
        }
    }

    // User-defined aliases — only emit in interactive context (D-010/D-004).
    if *params.context == Context::Interactive {
        for alias in params.user_aliases {
            // Only emit aliases tagged Interactive or All.
            if matches!(alias.context, AliasContext::Interactive | AliasContext::All) {
                out.push_str(&format!(
                    "  alias {}={}\n",
                    alias.name,
                    shell_quote(&alias.command)
                ));
            }
        }
    }

    // lx tab completions — add the completions dir to $fpath so compinit picks up
    // _lx automatically. Also call compdef conditionally for shells where compinit
    // has already run (e.g. macOS sources /etc/zshrc before ~/.zshrc).
    if let Some(completions_dir) = params.completions_zsh {
        out.push_str(&format!(
            "  fpath=({} $fpath)\n",
            shell_quote(completions_dir)
        ));
        // compdef is only available after compinit. Use (( $+functions[compdef] )) to
        // guard so this is safe regardless of where compinit sits in the user's zshrc.
        out.push_str("  (( $+functions[compdef] )) && compdef _lx lx\n");
    }

    // Editor preference from config — exported as $VISUAL only if not already set.
    // Uses ${VISUAL:-} so the user's existing env always wins over the config value.
    // Works with any editor binary: code, zed, vim, nano, etc.
    if let Some(editor) = params.editor {
        out.push_str(&format!(
            "  export VISUAL=${{VISUAL:-{}}}\n",
            shell_quote(editor)
        ));
    }

    // Bare subcommand wrappers — opt-in, interactive only, per-name collision-guarded.
    if params.bare_subcommands && *params.context == Context::Interactive {
        out.push_str(&generate_bare_wrappers());
    }

    // User-defined PATH entries — prepend regardless of context.
    for path in params.user_paths {
        out.push_str(&format!(
            "  export PATH={path_q}:\"$PATH\"\n",
            path_q = shell_quote(path)
        ));
    }

    // Not exported — must not leak into child shells or lx init would skip there too
    out.push_str(&format!("  typeset -g {}=1\n", env_vars::LYNX_INITIALIZED));
    out.push_str("fi\n");

    out
}

/// Safe lx subcommands exposed as bare shell functions when `bare_subcommands = true`.
///
/// Excluded intentionally:
/// - `init`, `config`, `run`, `install`, `sync`, `update`, `uninstall`, `setup`,
///   `event`, `migrate`, `path` — collide with common tools or are too generic
/// - `alias` — zsh builtin
/// - `jobs` — zsh builtin
/// - `prompt`, `git-state`, `kubectl-state`, `refresh-state`, `completions` — internal plumbing
const BARE_SUBCOMMANDS: &[&str] = &[
    "theme",
    "plugin",
    "doctor",
    "diag",
    "intro",
    "cron",
    "daemon",
    "rollback",
    "context",
    "benchmark",
    "tap",
    "browse",
    "audit",
    "dashboard",
    "examples",
];

/// Generate per-function collision-guarded shell wrappers for the safe subcommand set.
/// Each wrapper checks functions, commands, and aliases independently — a collision on
/// one name never prevents others from being registered.
fn generate_bare_wrappers() -> String {
    let mut out = String::new();
    out.push_str("  # bare subcommand wrappers (shell.bare_subcommands = true)\n");
    for cmd in BARE_SUBCOMMANDS {
        out.push_str(&format!(
            "  if (( ! $+functions[{cmd}] )) && (( ! $+commands[{cmd}] )) && (( ! $+aliases[{cmd}] )); then\n    function {cmd} {{ lx {cmd} \"$@\" }}\n  else\n    print -u2 'lynx: bare command \\'{cmd}\\' skipped — name already in use'\n  fi\n",
        ));
    }
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
            lynx_dir: "/home/user/.config/lynx",
            plugin_dir: "/home/user/.config/lynx/plugins",
            enabled_plugins: plugins,
            ls_colors: None,
            eza_colors: None,
            bsd_lscolors: None,
            syntax_highlight_styles: None,
            autosuggest_style: None,
            zle_hook_plugins: HashSet::new(),
            user_aliases: &[],
            user_paths: &[],
            editor: None,
            completions_zsh: None,
            bare_subcommands: false,
        }
    }

    #[test]
    fn output_contains_required_exports() {
        let plugins = vec!["git".to_string()];
        let script = generate_init_script(&base_params(&Context::Interactive, &plugins));
        assert!(script.contains(&format!("{}=", env_vars::LYNX_DIR)));
        assert!(script.contains(&format!("{}=interactive", env_vars::LYNX_CONTEXT)));
        assert!(script.contains(&format!("{}=", env_vars::LYNX_PLUGIN_DIR)));
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
        assert!(script.contains(&format!("{}=agent", env_vars::LYNX_CONTEXT)));
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
        assert!(script.contains(env_vars::LYNX_INITIALIZED));
    }

    #[test]
    fn zle_hook_plugin_emits_direct_source_not_eval_bridge() {
        let plugins = vec!["syntax-highlight".to_string(), "git".to_string()];
        let mut params = base_params(&Context::Interactive, &plugins);
        params.zle_hook_plugins = HashSet::from(["syntax-highlight".to_string()]);
        let script = generate_init_script(&params);
        // ZLE plugin must use direct source, not lynx_eval_plugin
        assert!(!script.contains("lynx_eval_plugin 'syntax-highlight'"));
        assert!(script.contains(&format!(
            "source \"${}/shell/init.zsh\"",
            env_vars::LYNX_PLUGIN_DIR
        )));
        // Non-ZLE plugin still uses eval bridge
        assert!(script.contains("lynx_eval_plugin 'git'"));
    }

    #[test]
    fn no_plugins_produces_valid_structure() {
        let script = generate_init_script(&base_params(&Context::Minimal, &[]));
        assert!(script.contains(&format!("{}=minimal", env_vars::LYNX_CONTEXT)));
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
        assert!(
            ls_pos > guard_pos,
            "LS_COLORS must be inside the init guard"
        );
        assert!(
            eza_pos > guard_pos,
            "EZA_COLORS must be inside the init guard"
        );
    }

    #[test]
    fn ls_colors_absent_when_not_provided() {
        let script = generate_init_script(&base_params(&Context::Interactive, &[]));
        assert!(
            !script.contains("LS_COLORS="),
            "LS_COLORS must not appear when not provided"
        );
    }

    #[test]
    fn hostname_inside_guard() {
        let script = generate_init_script(&base_params(&Context::Interactive, &[]));
        let guard_pos = script.find("if [[").unwrap();
        let hostname_pos = script.find("HOSTNAME").unwrap();
        assert!(
            hostname_pos > guard_pos,
            "HOSTNAME must be inside the init guard"
        );
    }

    #[test]
    fn user_aliases_emitted_in_interactive_context() {
        use lynx_config::schema::{AliasContext, UserAlias};
        let plugins = vec![];
        let aliases = vec![UserAlias {
            name: "gs".into(),
            command: "git status".into(),
            description: None,
            context: AliasContext::Interactive,
        }];
        let mut params = base_params(&Context::Interactive, &plugins);
        params.user_aliases = &aliases;
        let script = generate_init_script(&params);
        assert!(
            script.contains("alias gs="),
            "interactive alias must be emitted: {script}"
        );
    }

    #[test]
    fn user_aliases_not_emitted_in_agent_context() {
        use lynx_config::schema::{AliasContext, UserAlias};
        let plugins = vec![];
        let aliases = vec![UserAlias {
            name: "gs".into(),
            command: "git status".into(),
            description: None,
            context: AliasContext::Interactive,
        }];
        let mut params = base_params(&Context::Agent, &plugins);
        params.user_aliases = &aliases;
        let script = generate_init_script(&params);
        assert!(
            !script.contains("alias gs="),
            "aliases must NOT be emitted in agent context"
        );
    }

    #[test]
    fn user_paths_always_emitted() {
        let plugins = vec![];
        let paths = vec!["/usr/local/sbin".to_string()];
        let mut params = base_params(&Context::Agent, &plugins);
        params.user_paths = &paths;
        let script = generate_init_script(&params);
        assert!(
            script.contains("/usr/local/sbin"),
            "user paths must be emitted even in agent context"
        );
    }

    #[test]
    fn editor_exported_as_visual_when_set() {
        let mut params = base_params(&Context::Interactive, &[]);
        params.editor = Some("zed");
        let script = generate_init_script(&params);
        assert!(
            script.contains("export VISUAL=${VISUAL:-'zed'}"),
            "editor must be exported as VISUAL with env fallback: {script}"
        );
    }

    #[test]
    fn editor_not_exported_when_unset() {
        let script = generate_init_script(&base_params(&Context::Interactive, &[]));
        assert!(
            !script.contains("VISUAL="),
            "VISUAL must not appear when editor is not configured"
        );
    }

    #[test]
    fn bare_subcommands_disabled_emits_no_wrappers() {
        let mut params = base_params(&Context::Interactive, &[]);
        params.bare_subcommands = false;
        let script = generate_init_script(&params);
        assert!(
            !script.contains("bare subcommand wrappers"),
            "wrappers must not appear when bare_subcommands = false"
        );
        assert!(
            !script.contains("function theme"),
            "theme wrapper must not appear when disabled"
        );
    }

    #[test]
    fn bare_subcommands_enabled_emits_wrappers_in_interactive() {
        let mut params = base_params(&Context::Interactive, &[]);
        params.bare_subcommands = true;
        let script = generate_init_script(&params);
        assert!(
            script.contains("function theme"),
            "theme wrapper must appear when enabled in interactive: {script}"
        );
        assert!(
            script.contains("function plugin"),
            "plugin wrapper must appear: {script}"
        );
        assert!(
            script.contains("function doctor"),
            "doctor wrapper must appear: {script}"
        );
        // Each wrapper must be collision-guarded
        assert!(
            script.contains("$+functions[theme]"),
            "collision guard must check functions: {script}"
        );
        assert!(
            script.contains("$+commands[theme]"),
            "collision guard must check commands: {script}"
        );
        assert!(
            script.contains("$+aliases[theme]"),
            "collision guard must check aliases: {script}"
        );
    }

    #[test]
    fn bare_subcommands_not_emitted_in_agent_context() {
        let mut params = base_params(&Context::Agent, &[]);
        params.bare_subcommands = true;
        let script = generate_init_script(&params);
        assert!(
            !script.contains("function theme"),
            "bare wrappers must NOT appear in agent context"
        );
    }

    #[test]
    fn bare_subcommands_not_emitted_in_minimal_context() {
        let mut params = base_params(&Context::Minimal, &[]);
        params.bare_subcommands = true;
        let script = generate_init_script(&params);
        assert!(
            !script.contains("function theme"),
            "bare wrappers must NOT appear in minimal context"
        );
    }

    #[test]
    fn bare_wrappers_exclude_dangerous_subcommands() {
        let mut params = base_params(&Context::Interactive, &[]);
        params.bare_subcommands = true;
        let script = generate_init_script(&params);
        // These must never be exposed bare
        for forbidden in &["function init ", "function config ", "function run ", "function install ", "function alias ", "function jobs "] {
            assert!(
                !script.contains(forbidden),
                "forbidden bare command '{forbidden}' must not be emitted"
            );
        }
    }

    #[test]
    fn user_paths_prepend_to_path() {
        let plugins = vec![];
        let paths = vec!["/opt/custom/bin".to_string()];
        let mut params = base_params(&Context::Interactive, &plugins);
        params.user_paths = &paths;
        let script = generate_init_script(&params);
        assert!(
            script.contains("PATH='/opt/custom/bin':\"$PATH\""),
            "path must be prepended: {script}"
        );
    }
}
