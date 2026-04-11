use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LynxError {
    /// Config file is invalid or missing required fields.
    #[error("{0}\n  Fix: run `lx config validate` or `lx doctor` to diagnose")]
    Config(String),

    /// A plugin failed to load, validate, or activate.
    #[error("{0}\n  Fix: run `lx doctor` or `lx plugin list` to inspect plugin state")]
    Plugin(String),

    /// A theme file is invalid or missing.
    #[error("{0}\n  Fix: run `lx theme list` to see available themes, or `lx doctor`")]
    Theme(String),

    /// A shell integration error (generated script or context detection).
    #[error("{0}\n  Fix: run `lx doctor` to check shell integration")]
    Shell(String),

    /// A task scheduler error.
    #[error("{0}\n  Fix: run `lx task list` to inspect tasks, or `lx doctor`")]
    Task(String),

    /// A plugin manifest (plugin.toml) failed to parse or validate.
    #[error("{0}\n  Fix: check your plugin.toml against `lx plugin new <name>` template")]
    Manifest(String),

    /// An IO error with context: path + fix hint.
    #[error("{message}\n  Path: {path}\n  Fix: {fix}")]
    Io {
        message: String,
        path: PathBuf,
        fix: String,
    },

    /// Low-level IO error without path context (used by the `From<io::Error>` impl).
    #[error("{0}\n  Fix: run `lx doctor` to diagnose")]
    IoRaw(#[source] std::io::Error),
}

impl LynxError {
    /// Wrap an IO error with a file path and generate an appropriate fix hint.
    pub fn io(e: std::io::Error, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let message = match e.kind() {
            std::io::ErrorKind::NotFound => {
                format!("File not found: {}", path.display())
            }
            std::io::ErrorKind::PermissionDenied => {
                format!("Permission denied: {}", path.display())
            }
            _ => format!("IO error on {}: {e}", path.display()),
        };
        let fix = match e.kind() {
            std::io::ErrorKind::NotFound => {
                if path.extension().is_some_and(|ext| ext == "toml") {
                    "run `lx init` to regenerate config, or `lx doctor` to diagnose".into()
                } else {
                    "check that the path exists and is readable".into()
                }
            }
            std::io::ErrorKind::PermissionDenied => {
                format!("check file permissions: chmod 644 {}", path.display())
            }
            _ => "run `lx doctor` to diagnose".into(),
        };
        LynxError::Io { message, path, fix }
    }
}

/// Allow `?` from `std::io::Error` without a path.
/// Prefer `LynxError::io(e, path)` when a path is available.
impl From<std::io::Error> for LynxError {
    fn from(e: std::io::Error) -> Self {
        LynxError::IoRaw(e)
    }
}

pub type Result<T> = std::result::Result<T, LynxError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_variants_display_without_rust_internals() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "x");
        let cases: Vec<(&str, String)> = vec![
            ("Config", LynxError::Config("bad theme".into()).to_string()),
            (
                "Plugin",
                LynxError::Plugin("missing dep".into()).to_string(),
            ),
            (
                "Theme",
                LynxError::Theme("unknown theme".into()).to_string(),
            ),
            (
                "Shell",
                LynxError::Shell("context error".into()).to_string(),
            ),
            (
                "Task",
                LynxError::Task("cron parse fail".into()).to_string(),
            ),
            (
                "Manifest",
                LynxError::Manifest("missing field".into()).to_string(),
            ),
            ("Io", LynxError::io(io_err, "/tmp/config.toml").to_string()),
        ];
        for (name, msg) in &cases {
            assert!(!msg.is_empty(), "{name} display is empty");
            assert!(!msg.contains("unwrap"), "{name} leaks 'unwrap': {msg}");
            assert!(
                !msg.contains("called `Result"),
                "{name} leaks Result internals: {msg}"
            );
            assert!(msg.contains("Fix:"), "{name} missing Fix hint: {msg}");
        }
    }

    #[test]
    fn io_not_found_shows_path_and_fix() {
        let e = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
        let err = LynxError::io(e, "/home/user/.config/lynx/config.toml");
        let msg = err.to_string();
        assert!(msg.contains("config.toml"), "missing path: {msg}");
        assert!(msg.contains("Fix:"), "missing Fix hint: {msg}");
        assert!(
            msg.contains("lx"),
            "fix should suggest an lx command: {msg}"
        );
    }

    #[test]
    fn io_permission_denied_shows_chmod_hint() {
        let e = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let err = LynxError::io(e, "/etc/passwd");
        let msg = err.to_string();
        assert!(
            msg.contains("Permission denied"),
            "missing permission message: {msg}"
        );
        assert!(msg.contains("chmod"), "fix should suggest chmod: {msg}");
    }

    #[test]
    fn config_error_suggests_validate_command() {
        let err = LynxError::Config("active_theme must not be empty".into());
        let msg = err.to_string();
        assert!(
            msg.contains("lx config validate") || msg.contains("lx doctor"),
            "{msg}"
        );
    }
}
