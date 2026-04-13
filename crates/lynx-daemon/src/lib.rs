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

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_status_display() {
        assert_eq!(ServiceStatus::Running.to_string(), "running");
        assert_eq!(ServiceStatus::Stopped.to_string(), "stopped");
        assert_eq!(
            ServiceStatus::Unknown("something".into()).to_string(),
            "unknown (something)"
        );
    }

    #[test]
    fn service_status_eq() {
        assert_eq!(ServiceStatus::Running, ServiceStatus::Running);
        assert_ne!(ServiceStatus::Running, ServiceStatus::Stopped);
    }

    #[test]
    fn platform_backend_returns_backend() {
        // Should not panic on supported platforms
        let _backend = platform_backend();
    }
}
