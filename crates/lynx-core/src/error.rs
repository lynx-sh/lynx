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
    #[error("{0}\n  Fix: run `lx cron list` to inspect tasks, or `lx doctor`")]
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

    /// A named item (plugin, theme, workflow, tap, etc.) was not found.
    ///
    /// Use this instead of raw `bail!()` for any "X does not exist" case.
    /// The renderer will show the item type and name prominently.
    #[error("{item_type} '{name}' not found\n  Fix: {hint}")]
    NotFound {
        item_type: String,
        name: String,
        hint: String,
    },

    /// An item is already installed / enabled and cannot be installed again.
    #[error("{0} is already installed\n  Fix: use `lx plugin reinstall` to force-reinstall, or `lx plugin remove` first")]
    AlreadyInstalled(String),

    /// An item is not installed but an operation requires it to be.
    #[error(
        "{0} is not installed\n  Fix: run `lx plugin add {0}` or `lx install {0}` to install it"
    )]
    NotInstalled(String),

    /// A registry operation failed (fetch, parse, authentication).
    #[error(
        "{0}\n  Fix: run `lx tap update` to refresh indexes, or check your network connection"
    )]
    Registry(String),

    /// A workflow error (missing file, invalid schema, execution failure).
    #[error("{0}\n  Fix: run `lx run list` to see available workflows, or check the TOML in ~/.config/lynx/workflows/")]
    Workflow(String),

    /// A daemon error (service management, IPC, startup).
    #[error("{0}\n  Fix: run `lx daemon status` to check the daemon, or `lx daemon restart`")]
    Daemon(String),
}

impl LynxError {
    /// Create a "command not found" error with a hint to run the parent command for help.
    pub fn unknown_command(command: &str, parent: &str) -> Self {
        LynxError::NotFound {
            item_type: "Command".into(),
            name: command.into(),
            hint: format!("run `lx {parent}` for help"),
        }
    }

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

    /// Extract the fix hint for this error, if any.
    ///
    /// Used by the CLI renderer to display the hint on a separate styled line.
    /// Returns `None` for raw IO errors where the hint is embedded in the message.
    pub fn hint(&self) -> Option<&str> {
        match self {
            LynxError::Config(_) => Some("run `lx config validate` or `lx doctor` to diagnose"),
            LynxError::Plugin(_) => {
                Some("run `lx doctor` or `lx plugin list` to inspect plugin state")
            }
            LynxError::Theme(_) => {
                Some("run `lx theme list` to see available themes, or `lx doctor`")
            }
            LynxError::Shell(_) => Some("run `lx doctor` to check shell integration"),
            LynxError::Task(_) => Some("run `lx cron list` to inspect tasks, or `lx doctor`"),
            LynxError::Manifest(_) => {
                Some("check your plugin.toml against `lx plugin new <name>` template")
            }
            LynxError::Io { fix, .. } => Some(fix.as_str()),
            LynxError::IoRaw(_) => Some("run `lx doctor` to diagnose"),
            LynxError::NotFound { hint, .. } => Some(hint.as_str()),
            LynxError::AlreadyInstalled(_) => {
                Some("use `lx plugin reinstall` to force-reinstall, or `lx plugin remove` first")
            }
            LynxError::NotInstalled(name) => {
                // Can't return a reference to a temporary, so return a static hint.
                // The renderer will use format_hint() for dynamic cases.
                let _ = name;
                Some("run `lx plugin add <name>` or `lx install <name>` to install it")
            }
            LynxError::Registry(_) => {
                Some("run `lx tap update` to refresh indexes, or check your network connection")
            }
            LynxError::Workflow(_) => Some(
                "run `lx run list` to see available workflows, or check ~/.config/lynx/workflows/",
            ),
            LynxError::Daemon(_) => {
                Some("run `lx daemon status` to check the daemon, or `lx daemon restart`")
            }
        }
    }

    /// Extract just the primary message (without the embedded Fix: line).
    pub fn message(&self) -> String {
        match self {
            LynxError::Config(m)
            | LynxError::Plugin(m)
            | LynxError::Theme(m)
            | LynxError::Shell(m)
            | LynxError::Task(m)
            | LynxError::Manifest(m)
            | LynxError::Registry(m)
            | LynxError::Workflow(m)
            | LynxError::Daemon(m) => m.clone(),
            LynxError::Io { message, .. } => message.clone(),
            LynxError::IoRaw(e) => e.to_string(),
            LynxError::NotFound {
                item_type, name, ..
            } => {
                format!("{item_type} '{name}' not found")
            }
            LynxError::AlreadyInstalled(name) => format!("'{name}' is already installed"),
            LynxError::NotInstalled(name) => format!("'{name}' is not installed"),
        }
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
