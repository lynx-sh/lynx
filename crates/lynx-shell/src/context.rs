use lynx_core::env_vars;
use lynx_core::types::Context;

/// Canonical context override env var — use `env_vars::LYNX_CONTEXT` directly.
pub const CONTEXT_OVERRIDE_ENV: &str = env_vars::LYNX_CONTEXT;

/// Canonical env vars that indicate agent context.
pub const AGENT_ENV_VARS: &[&str] = &[env_vars::CLAUDECODE, env_vars::CURSOR_CLI];

/// Canonical env vars that indicate minimal context.
pub const MINIMAL_ENV_VARS: &[&str] = &[env_vars::CI];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectionMethod {
    Override,
    AgentEnv(&'static str),
    MinimalEnv(&'static str),
    DefaultInteractive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectionOutcome {
    pub context: Context,
    pub method: DetectionMethod,
}

/// Detect the current [`Context`] from the process environment.
///
/// Priority order:
/// 1. Any `AGENT_ENV_VARS` set → `Agent`  (ground truth — beats inherited LYNX_CONTEXT)
/// 2. `LYNX_CONTEXT` explicit override
/// 3. Any `MINIMAL_ENV_VARS` set → `Minimal`
/// 4. Default → `Interactive`
///
/// Agent env vars rank first because `LYNX_CONTEXT=interactive` may be inherited from
/// a parent interactive shell. A host tool's env var (e.g. `CLAUDECODE=1`) is the only
/// reliable signal that the current session is an agent session.
pub fn detect_context() -> Context {
    detect_context_outcome().context
}

pub fn detect_context_outcome() -> DetectionOutcome {
    // Agent env vars are ground truth — checked before LYNX_CONTEXT so an inherited
    // interactive value from a parent shell cannot suppress agent detection.
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

    if let Some(context) = override_context() {
        return DetectionOutcome {
            context,
            method: DetectionMethod::Override,
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
    Context::parse(val.to_lowercase().as_str())
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
        let _g1 = EnvGuard::unset(&[env_vars::LYNX_CONTEXT]);
        let _g2 = EnvGuard::set(&[(env_vars::CLAUDECODE, "1")]);
        assert_eq!(detect_context(), Context::Agent);
    }

    #[test]
    fn agent_env_beats_inherited_lynx_context_interactive() {
        // CLAUDECODE=1 must win even when LYNX_CONTEXT=interactive is inherited from
        // a parent interactive shell — agent env is ground truth.
        let _g = EnvGuard::set(&[
            (env_vars::LYNX_CONTEXT, "interactive"),
            (env_vars::CLAUDECODE, "1"),
        ]);
        assert_eq!(detect_context(), Context::Agent);
    }

    #[test]
    fn cursor_session_set_returns_agent() {
        let _g1 = EnvGuard::unset(&[env_vars::LYNX_CONTEXT]);
        let _g2 = EnvGuard::set(&[(env_vars::CURSOR_CLI, "abc")]);
        assert_eq!(detect_context(), Context::Agent);
    }

    #[test]
    fn lynx_context_override_agent() {
        let _g1 = EnvGuard::unset(&["CLAUDECODE", "CURSOR_CLI"]);
        let _g2 = EnvGuard::set(&[(env_vars::LYNX_CONTEXT, "agent")]);
        assert_eq!(detect_context(), Context::Agent);
    }

    #[test]
    fn lynx_context_override_minimal() {
        let _g1 = EnvGuard::unset(&["CLAUDECODE", "CURSOR_CLI"]);
        let _g2 = EnvGuard::set(&[(env_vars::LYNX_CONTEXT, "minimal")]);
        assert_eq!(detect_context(), Context::Minimal);
    }

    #[test]
    fn lynx_context_override_interactive() {
        let _g1 = EnvGuard::unset(&["CLAUDECODE", "CURSOR_CLI"]);
        let _g2 = EnvGuard::set(&[(env_vars::LYNX_CONTEXT, "interactive")]);
        assert_eq!(detect_context(), Context::Interactive);
    }

    #[test]
    fn ci_with_no_tty_returns_minimal() {
        let _g1 = EnvGuard::unset(&[
            env_vars::LYNX_CONTEXT,
            env_vars::CLAUDECODE,
            env_vars::CURSOR_CLI,
        ]);
        let _g2 = EnvGuard::set(&[(env_vars::CI, "true")]);
        assert_eq!(detect_context(), Context::Minimal);
    }

    #[test]
    fn unknown_override_falls_back_to_auto_detect() {
        let _g1 = EnvGuard::set(&[
            (env_vars::LYNX_CONTEXT, "unknown"),
            (env_vars::CLAUDECODE, "1"),
        ]);
        assert_eq!(detect_context(), Context::Agent);
    }

    #[test]
    fn outcome_reports_detecting_env_var() {
        let _g1 = EnvGuard::unset(&[
            env_vars::LYNX_CONTEXT,
            env_vars::CLAUDECODE,
            env_vars::CURSOR_CLI,
            env_vars::CI,
        ]);
        let _g2 = EnvGuard::set(&[(env_vars::CURSOR_CLI, "session-123")]);
        let out = detect_context_outcome();
        assert_eq!(out.context, Context::Agent);
        assert_eq!(out.method, DetectionMethod::AgentEnv("CURSOR_CLI"));
    }

    #[test]
    fn docs_use_canonical_agent_env_vars() {
        let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf();

        let readme =
            std::fs::read_to_string(workspace_root.join("README.md")).expect("read README");
        assert!(readme.contains(env_vars::CLAUDECODE));
        assert!(readme.contains(env_vars::CURSOR_CLI));
        assert!(!readme.contains("CLAUDE_CODE"));
        assert!(!readme.contains("CURSOR_SESSION"));

        let architecture = std::fs::read_to_string(workspace_root.join("docs/architecture.md"))
            .expect("read architecture doc");
        assert!(architecture.contains(env_vars::CLAUDECODE));
        assert!(architecture.contains(env_vars::CURSOR_CLI));
        assert!(!architecture.contains("CLAUDE_CODE"));
        assert!(!architecture.contains("CURSOR_SESSION"));
    }
}
