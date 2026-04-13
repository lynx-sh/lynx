use lynx_core::env_vars;

/// Safe mode boot output for `lx init` when the config is invalid.
///
/// Instead of crashing the shell, Lynx emits a minimal init script that:
/// - Sets LYNX_SAFE_MODE=1 so the prompt can show a warning badge
/// - Prints a warning visible to the user (via `print -u2`)
/// - Skips all plugin loading and theme rendering
///
/// The shell remains fully functional — only Lynx extras are disabled.
pub fn generate_safemode_script(reason: &str) -> String {
    // Sanitize the reason string for embedding in a zsh double-quoted string.
    let safe_reason = reason.replace('"', "'").replace('\n', " | ");

    format!(
        r#"# Lynx safe mode — config error detected
export LYNX_SAFE_MODE=1
export LYNX_CONTEXT=minimal
print -u2 "Lynx: starting in safe mode due to config error:"
print -u2 "  {safe_reason}"
print -u2 "  Fix: run 'lx doctor' to diagnose, or 'lx config validate' to check your config."
"#,
    )
}

/// Returns true if the current shell session is in safe mode.
/// Check `$LYNX_SAFE_MODE` env var (set to "1" in safe mode).
pub fn is_safe_mode() -> bool {
    std::env::var(env_vars::LYNX_SAFE_MODE).as_deref() == Ok("1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safemode_script_sets_env_var() {
        let script = generate_safemode_script("active_theme must not be empty");
        assert!(
            script.contains("LYNX_SAFE_MODE=1"),
            "must set LYNX_SAFE_MODE: {script}"
        );
        assert!(
            script.contains("LYNX_CONTEXT=minimal"),
            "must set minimal context: {script}"
        );
    }

    #[test]
    fn safemode_script_prints_fix_hint() {
        let script = generate_safemode_script("bad config");
        assert!(
            script.contains("lx doctor"),
            "must suggest lx doctor: {script}"
        );
    }

    #[test]
    fn safemode_script_includes_reason() {
        let script = generate_safemode_script("active_theme is empty");
        assert!(script.contains("active_theme is empty"), "{script}");
    }

    #[test]
    fn reason_quotes_sanitized() {
        let script = generate_safemode_script(r#"theme "bad" set"#);
        // The sanitized reason replaces " with '
        assert!(script.contains("theme 'bad' set"), "{script}");
    }
}
