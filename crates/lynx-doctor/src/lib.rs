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
#[derive(Debug, PartialEq)]
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
