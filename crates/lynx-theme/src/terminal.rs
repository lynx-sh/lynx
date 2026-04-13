use std::sync::OnceLock;
use std::{cell::Cell, thread_local};

/// Terminal color capability level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TermCapability {
    /// 24-bit truecolor (e.g. $COLORTERM=truecolor or 24bit).
    TrueColor,
    /// 256-color xterm palette.
    Ansi256,
    /// Basic 16 ANSI colors.
    Basic16,
    /// No color output (dumb terminals, pipes without $FORCE_COLOR).
    None,
}

static CAPABILITY: OnceLock<TermCapability> = OnceLock::new();
thread_local! {
    static OVERRIDE: Cell<Option<TermCapability>> = const { Cell::new(None) };
}

/// Detect and cache terminal color capability. Runs only once.
pub fn capability() -> TermCapability {
    if let Some(cap) = OVERRIDE.with(Cell::get) {
        return cap;
    }
    *CAPABILITY.get_or_init(detect)
}

/// Force a specific capability for the current thread.
///
/// Primarily useful in tests to avoid env var dependence under parallel test execution.
pub fn override_capability(cap: TermCapability) {
    OVERRIDE.with(|slot| slot.set(Some(cap)));
}

/// Clear forced terminal capability override.
///
/// Primarily used by tests that need to restore default detector behavior.
pub fn clear_capability_override() {
    OVERRIDE.with(|slot| slot.set(None));
}

fn detect() -> TermCapability {
    // $COLORTERM=truecolor or 24bit → TrueColor
    if let Ok(val) = std::env::var("COLORTERM") {
        if val == "truecolor" || val == "24bit" {
            return TermCapability::TrueColor;
        }
    }

    // $TERM contains 256color → Ansi256
    if let Ok(term) = std::env::var("TERM") {
        if term.contains("256color") {
            return TermCapability::Ansi256;
        }
        // Dumb terminal → None
        if term == "dumb" {
            return TermCapability::None;
        }
    }

    // $NO_COLOR set → None (https://no-color.org/)
    if std::env::var("NO_COLOR").is_ok() {
        return TermCapability::None;
    }

    // Default: basic 16-color ANSI
    TermCapability::Basic16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_runs_without_panic() {
        // Just assert it returns a valid variant (env vars vary in CI).
        let _cap = detect();
    }
}
