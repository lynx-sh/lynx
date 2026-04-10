use lynx_core::types::Context;

/// Env vars whose presence signals an agent/AI coding environment.
/// Add new tools here — detection is automatic everywhere.
static AGENT_ENV_VARS: &[&str] = &[
    "CLAUDE_CODE",
    "CURSOR_SESSION",
    "CODEIUM_SESSION",
    "COPILOT_AGENT",
    "WINDSURF_AGENT",
];

/// Detect the current [`Context`] from the process environment.
///
/// Priority order:
/// 1. `LYNX_CONTEXT` env override (respects user's explicit choice)
/// 2. Any `AGENT_ENV_VARS` set → `Agent`
/// 3. No TTY (non-interactive shell) → `Minimal`
/// 4. Default → `Interactive`
pub fn detect_context() -> Context {
    // 1. Explicit override
    if let Ok(val) = std::env::var("LYNX_CONTEXT") {
        match val.to_lowercase().as_str() {
            "agent" => return Context::Agent,
            "minimal" => return Context::Minimal,
            "interactive" => return Context::Interactive,
            _ => {} // unknown value — fall through to auto-detect
        }
    }

    // 2. Agent env vars
    if AGENT_ENV_VARS
        .iter()
        .any(|var| std::env::var_os(var).is_some())
    {
        return Context::Agent;
    }

    // 3. Non-interactive (no TTY)
    if !is_interactive() {
        return Context::Minimal;
    }

    // 4. Default
    Context::Interactive
}

/// Returns true when stdin is a TTY (i.e. an interactive shell session).
fn is_interactive() -> bool {
    // Use the CI env var as a proxy for non-interactive; also check SSH_TTY presence.
    // In tests we can override via LYNX_CONTEXT, so this is only a real-world heuristic.
    if std::env::var_os("CI").is_some() {
        return false;
    }
    // Treat SSH sessions without a TTY as non-interactive
    if std::env::var_os("SSH_TTY").is_none() && std::env::var_os("TERM").is_none() {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    /// Guard that cleans up env vars after a test.
    struct EnvGuard(Vec<(&'static str, Option<std::ffi::OsString>)>);
    impl EnvGuard {
        fn set(vars: &[(&'static str, &str)]) -> Self {
            let saved = vars
                .iter()
                .map(|(k, _)| (*k, env::var_os(k)))
                .collect();
            for (k, v) in vars {
                env::set_var(k, v);
            }
            // Remove vars that shouldn't be set
            EnvGuard(saved)
        }
        fn unset(vars: &[&'static str]) -> Self {
            let saved = vars.iter().map(|k| (*k, env::var_os(k))).collect();
            for k in vars {
                env::remove_var(k);
            }
            EnvGuard(saved)
        }
    }
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (k, v) in &self.0 {
                match v {
                    Some(val) => env::set_var(k, val),
                    None => env::remove_var(k),
                }
            }
        }
    }

    #[test]
    fn claude_code_set_returns_agent() {
        let _g1 = EnvGuard::unset(&["LYNX_CONTEXT"]);
        let _g2 = EnvGuard::set(&[("CLAUDE_CODE", "1")]);
        assert_eq!(detect_context(), Context::Agent);
    }

    #[test]
    fn cursor_session_set_returns_agent() {
        let _g1 = EnvGuard::unset(&["LYNX_CONTEXT"]);
        let _g2 = EnvGuard::set(&[("CURSOR_SESSION", "abc")]);
        assert_eq!(detect_context(), Context::Agent);
    }

    #[test]
    fn lynx_context_override_agent() {
        let _g = EnvGuard::set(&[("LYNX_CONTEXT", "agent")]);
        assert_eq!(detect_context(), Context::Agent);
    }

    #[test]
    fn lynx_context_override_minimal() {
        let _g = EnvGuard::set(&[("LYNX_CONTEXT", "minimal")]);
        assert_eq!(detect_context(), Context::Minimal);
    }

    #[test]
    fn lynx_context_override_interactive() {
        let _g = EnvGuard::set(&[("LYNX_CONTEXT", "interactive")]);
        assert_eq!(detect_context(), Context::Interactive);
    }

    #[test]
    fn ci_with_no_tty_returns_minimal() {
        let _g1 = EnvGuard::unset(&["LYNX_CONTEXT", "CLAUDE_CODE", "CURSOR_SESSION",
                                    "CODEIUM_SESSION", "COPILOT_AGENT", "WINDSURF_AGENT", "TERM"]);
        let _g2 = EnvGuard::set(&[("CI", "true")]);
        assert_eq!(detect_context(), Context::Minimal);
    }
}
