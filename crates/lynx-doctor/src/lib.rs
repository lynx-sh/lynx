mod checks;

/// A single health-check result.
#[derive(Debug)]
pub struct Check {
    pub name: &'static str,
    pub status: Status,
    pub detail: String,
    pub fix: Option<String>,
}

/// Health-check outcome.
#[derive(Debug, PartialEq, Eq)]
pub enum Status {
    Pass,
    Warn,
    Fail,
}

impl Status {
    pub fn symbol(&self) -> &'static str {
        match self {
            Status::Pass => "\u{2713}",
            Status::Warn => "\u{26a0}",
            Status::Fail => "\u{2717}",
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Status::Pass => "pass",
            Status::Warn => "warn",
            Status::Fail => "fail",
        }
    }
}

/// Run every health check and return results in display order.
pub fn run_all() -> Vec<Check> {
    checks::run_all()
}

/// Parse a zsh --version string into (major, minor).
pub fn parse_zsh_version(s: &str) -> Option<(u32, u32)> {
    checks::parse_zsh_version(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_symbol() {
        assert_eq!(Status::Pass.symbol(), "\u{2713}");
        assert_eq!(Status::Warn.symbol(), "\u{26a0}");
        assert_eq!(Status::Fail.symbol(), "\u{2717}");
    }

    #[test]
    fn status_label() {
        assert_eq!(Status::Pass.label(), "pass");
        assert_eq!(Status::Warn.label(), "warn");
        assert_eq!(Status::Fail.label(), "fail");
    }

    #[test]
    fn parse_zsh_version_standard() {
        assert_eq!(
            parse_zsh_version("zsh 5.9 (x86_64-apple-darwin23.0)"),
            Some((5, 9))
        );
    }

    #[test]
    fn parse_zsh_version_minimal() {
        assert_eq!(parse_zsh_version("zsh 5.8"), Some((5, 8)));
    }

    #[test]
    fn parse_zsh_version_invalid() {
        assert_eq!(parse_zsh_version("bash 5.2"), None);
        assert_eq!(parse_zsh_version(""), None);
        assert_eq!(parse_zsh_version("zsh"), None);
    }

    #[test]
    fn run_all_returns_checks() {
        let checks = run_all();
        // Should always return at least some checks
        assert!(!checks.is_empty());
    }
}
