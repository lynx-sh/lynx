use lynx_core::types::Context;

/// Canonical context override env var.
pub const CONTEXT_OVERRIDE_ENV: &str = "LYNX_CONTEXT";

/// Canonical env vars that indicate agent context.
pub const AGENT_ENV_VARS: &[&str] = &["CLAUDE_CODE", "CURSOR_SESSION"];

/// Canonical env vars that indicate minimal context.
pub const MINIMAL_ENV_VARS: &[&str] = &["CI"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectionMethod {
    Override,
    AgentEnv(&'static str),
    MinimalEnv(&'static str),
    DefaultInteractive,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DetectionOutcome {
    pub context: Context,
    pub method: DetectionMethod,
}

/// Detect the current [`Context`] from the process environment.
///
/// Priority order:
/// 1. `LYNX_CONTEXT` env override (respects explicit choice)
/// 2. Any canonical `AGENT_ENV_VARS` set → `Agent`
/// 3. Any canonical `MINIMAL_ENV_VARS` set → `Minimal`
/// 4. Default → `Interactive`
pub fn detect_context() -> Context {
    detect_context_outcome().context
}

/// Detect context for use by `lx init` — ignores any inherited `LYNX_CONTEXT` env var.
///
/// `lx init` is the process that *writes* `LYNX_CONTEXT`, so reading it back as an
/// override would cause parent-shell context to bleed into child shells (e.g. an
/// interactive terminal exporting `LYNX_CONTEXT=interactive` then Claude Code opening
/// a new terminal that inherits it, suppressing agent detection).
///
/// User-facing `--context` flag on `lx init` is handled by the caller after this.
pub fn detect_context_for_init() -> Context {
    // Check agent env vars first — these are injected by the host tool (Claude Code, Cursor, etc.)
    if let Some(_var) = AGENT_ENV_VARS
        .iter()
        .copied()
        .find(|var| std::env::var_os(var).is_some())
    {
        return Context::Agent;
    }

    if let Some(_var) = MINIMAL_ENV_VARS
        .iter()
        .copied()
        .find(|var| std::env::var_os(var).is_some())
    {
        return Context::Minimal;
    }

    Context::Interactive
}

pub fn detect_context_outcome() -> DetectionOutcome {
    if let Some(context) = override_context() {
        return DetectionOutcome {
            context,
            method: DetectionMethod::Override,
        };
    }

    if let Some(var) = AGENT_ENV_VARS
        .iter()
        .copied()
        .find(|var| std::env::var_os(var).is_some())
    {
        return DetectionOutcome {
            context: Context::Agent,
            method: DetectionMethod::AgentEnv(var),
        };
    }

    if let Some(var) = MINIMAL_ENV_VARS
        .iter()
        .copied()
        .find(|var| std::env::var_os(var).is_some())
    {
        return DetectionOutcome {
            context: Context::Minimal,
            method: DetectionMethod::MinimalEnv(var),
        };
    }

    DetectionOutcome {
        context: Context::Interactive,
        method: DetectionMethod::DefaultInteractive,
    }
}

fn override_context() -> Option<Context> {
    let val = std::env::var(CONTEXT_OVERRIDE_ENV).ok()?;
    match val.to_lowercase().as_str() {
        "agent" => Some(Context::Agent),
        "minimal" => Some(Context::Minimal),
        "interactive" => Some(Context::Interactive),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    /// Guard that cleans up env vars after a test.
    struct EnvGuard(Vec<(&'static str, Option<std::ffi::OsString>)>);
    impl EnvGuard {
        fn set(vars: &[(&'static str, &str)]) -> Self {
            let saved = vars.iter().map(|(k, _)| (*k, env::var_os(k))).collect();
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
    fn detect_for_init_ignores_inherited_lynx_context() {
        // Simulate a child shell inheriting LYNX_CONTEXT=interactive from a parent
        // while Claude Code env var is also present.
        let _g1 = EnvGuard::set(&[("LYNX_CONTEXT", "interactive"), ("CLAUDE_CODE", "1")]);
        // detect_context() would return Interactive (override wins)
        assert_eq!(detect_context(), Context::Interactive);
        // detect_context_for_init() must return Agent (ignores inherited LYNX_CONTEXT)
        assert_eq!(detect_context_for_init(), Context::Agent);
    }

    #[test]
    fn detect_for_init_falls_back_to_interactive_when_no_agent_env() {
        let _g = EnvGuard::unset(&["LYNX_CONTEXT", "CLAUDE_CODE", "CURSOR_SESSION", "CI"]);
        assert_eq!(detect_context_for_init(), Context::Interactive);
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
        let _g1 = EnvGuard::unset(&["LYNX_CONTEXT", "CLAUDE_CODE", "CURSOR_SESSION"]);
        let _g2 = EnvGuard::set(&[("CI", "true")]);
        assert_eq!(detect_context(), Context::Minimal);
    }

    #[test]
    fn unknown_override_falls_back_to_auto_detect() {
        let _g1 = EnvGuard::set(&[("LYNX_CONTEXT", "unknown"), ("CLAUDE_CODE", "1")]);
        assert_eq!(detect_context(), Context::Agent);
    }

    #[test]
    fn outcome_reports_detecting_env_var() {
        let _g1 = EnvGuard::unset(&["LYNX_CONTEXT", "CLAUDE_CODE", "CURSOR_SESSION", "CI"]);
        let _g2 = EnvGuard::set(&[("CURSOR_SESSION", "session-123")]);
        let out = detect_context_outcome();
        assert_eq!(out.context, Context::Agent);
        assert_eq!(out.method, DetectionMethod::AgentEnv("CURSOR_SESSION"));
    }
}
