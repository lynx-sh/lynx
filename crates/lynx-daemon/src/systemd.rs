use crate::{ServiceBackend, ServiceStatus};
use anyhow::{Context, Result};
use lynx_core::{brand, env_vars, paths};
use std::path::PathBuf;

pub struct SystemdBackend {
    unit_path: PathBuf,
    binary_path: PathBuf,
}

impl SystemdBackend {
    pub fn new() -> Self {
        let config_home = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
            let home = std::env::var(env_vars::HOME).unwrap_or_default();
            format!("{home}/.config")
        });

        Self {
            unit_path: PathBuf::from(&config_home)
                .join("systemd/user")
                .join(brand::SYSTEMD_SERVICE),
            binary_path: Self::find_binary(),
        }
    }

    fn find_binary() -> PathBuf {
        if let Ok(p) = std::env::var(env_vars::LYNX_DAEMON_BIN) {
            return PathBuf::from(p);
        }
        if let Ok(output) = std::process::Command::new("which")
            .arg(brand::DAEMON_NAME)
            .output()
        {
            if output.status.success() {
                let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !s.is_empty() {
                    return PathBuf::from(s);
                }
            }
        }
        paths::bin_dir().join(brand::DAEMON_NAME)
    }

    fn unit_content(&self) -> String {
        format!(
            r#"[Unit]
Description=Lynx shell framework daemon
After=default.target

[Service]
Type=simple
ExecStart={bin}
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
"#,
            bin = self.binary_path.display()
        )
    }

    fn systemctl(&self, args: &[&str]) -> Result<std::process::Output> {
        std::process::Command::new("systemctl")
            .arg("--user")
            .args(args)
            .output()
            .context("systemctl failed")
    }
}

impl Default for SystemdBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceBackend for SystemdBackend {
    fn install(&self) -> Result<()> {
        if let Some(parent) = self.unit_path.parent() {
            std::fs::create_dir_all(parent)
                .context("failed to create systemd user unit directory")?;
        }

        std::fs::write(&self.unit_path, self.unit_content())
            .context("failed to write systemd unit file")?;

        self.systemctl(&["daemon-reload"])
            .context("systemctl daemon-reload failed")?;

        let out = self
            .systemctl(&["enable", "--now", brand::SYSTEMD_SERVICE])
            .context("systemctl enable failed")?;

        if !out.status.success() {
            return Err(lynx_core::error::LynxError::Daemon(format!(
                "systemctl enable failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ))
            .into());
        }

        Ok(())
    }

    fn uninstall(&self) -> Result<()> {
        let _ = self.systemctl(&["disable", "--now", brand::SYSTEMD_SERVICE]);

        if self.unit_path.exists() {
            std::fs::remove_file(&self.unit_path).context("failed to remove systemd unit file")?;
        }

        let _ = self.systemctl(&["daemon-reload"]);
        Ok(())
    }

    fn start(&self) -> Result<()> {
        let out = self.systemctl(&["start", brand::SYSTEMD_SERVICE])?;
        if !out.status.success() {
            return Err(lynx_core::error::LynxError::Daemon(format!(
                "systemctl start failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ))
            .into());
        }
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        let out = self.systemctl(&["stop", brand::SYSTEMD_SERVICE])?;
        if !out.status.success() {
            return Err(lynx_core::error::LynxError::Daemon(format!(
                "systemctl stop failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ))
            .into());
        }
        Ok(())
    }

    fn restart(&self) -> Result<()> {
        let out = self.systemctl(&["restart", brand::SYSTEMD_SERVICE])?;
        if !out.status.success() {
            return Err(lynx_core::error::LynxError::Daemon(format!(
                "systemctl restart failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ))
            .into());
        }
        Ok(())
    }

    fn status(&self) -> Result<ServiceStatus> {
        let out = self.systemctl(&["is-active", brand::SYSTEMD_SERVICE])?;
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();

        Ok(match text.as_str() {
            "active" => ServiceStatus::Running,
            "inactive" | "failed" => ServiceStatus::Stopped,
            other => ServiceStatus::Unknown(other.to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_content_has_required_sections() {
        let backend = SystemdBackend::new();
        let content = backend.unit_content();
        assert!(content.contains("[Unit]"));
        assert!(content.contains("[Service]"));
        assert!(content.contains("[Install]"));
        assert!(content.contains("Restart=on-failure"));
    }

    #[test]
    fn unit_content_has_binary() {
        let backend = SystemdBackend::new();
        let content = backend.unit_content();
        assert!(content.contains("ExecStart="));
    }
}
