pub mod launchd;
pub mod systemd;

pub use launchd::LaunchdBackend;
pub use systemd::SystemdBackend;

/// Abstraction over platform service managers.
pub trait ServiceBackend {
    fn install(&self) -> anyhow::Result<()>;
    fn uninstall(&self) -> anyhow::Result<()>;
    fn start(&self) -> anyhow::Result<()>;
    fn stop(&self) -> anyhow::Result<()>;
    fn restart(&self) -> anyhow::Result<()>;
    fn status(&self) -> anyhow::Result<ServiceStatus>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum ServiceStatus {
    Running,
    Stopped,
    Unknown(String),
}

impl std::fmt::Display for ServiceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running => write!(f, "running"),
            Self::Stopped => write!(f, "stopped"),
            Self::Unknown(s) => write!(f, "unknown ({s})"),
        }
    }
}

/// Return the correct backend for the current platform.
pub fn platform_backend() -> Box<dyn ServiceBackend> {
    #[cfg(target_os = "macos")]
    return Box::new(LaunchdBackend::new());

    #[cfg(target_os = "linux")]
    return Box::new(SystemdBackend::new());

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    panic!("Lynx daemon: unsupported platform");
}
