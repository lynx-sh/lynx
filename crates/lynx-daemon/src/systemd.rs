use crate::{ServiceBackend, ServiceStatus};
use anyhow::{Context, Result};
use std::path::PathBuf;

const SERVICE_NAME: &str = "lynx-daemon.service";

pub struct SystemdBackend {
    unit_path: PathBuf,
    binary_path: PathBuf,
}

impl SystemdBackend {
    pub fn new() -> Self {
        let config_home = std::env::var("XDG_CONFIG_HOME")
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_default();
                format!("{home}/.config")
            });

        Self {
            unit_path: PathBuf::from(&config_home)
                .join("systemd/user")
                .join(SERVICE_NAME),
            binary_path: Self::find_binary(),
        }
    }

    fn find_binary() -> PathBuf {
        if let Ok(p) = std::env::var("LYNX_DAEMON_BIN") {
            return PathBuf::from(p);
        }
        if let Ok(output) = std::process::Command::new("which")
            .arg("lynx-daemon")
            .output()
        {
            if output.status.success() {
                let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !s.is_empty() {
                    return PathBuf::from(s);
                }
            }
        }
        PathBuf::from("/usr/local/bin/lynx-daemon")
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

        let out = self.systemctl(&["enable", "--now", SERVICE_NAME])
            .context("systemctl enable failed")?;

        if !out.status.success() {
            anyhow::bail!(
                "systemctl enable failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }

        Ok(())
    }

    fn uninstall(&self) -> Result<()> {
        let _ = self.systemctl(&["disable", "--now", SERVICE_NAME]);

        if self.unit_path.exists() {
            std::fs::remove_file(&self.unit_path)
                .context("failed to remove systemd unit file")?;
        }

        let _ = self.systemctl(&["daemon-reload"]);
        Ok(())
    }

    fn start(&self) -> Result<()> {
        let out = self.systemctl(&["start", SERVICE_NAME])?;
        if !out.status.success() {
            anyhow::bail!(
                "systemctl start failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        let out = self.systemctl(&["stop", SERVICE_NAME])?;
        if !out.status.success() {
            anyhow::bail!(
                "systemctl stop failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Ok(())
    }

    fn restart(&self) -> Result<()> {
        let out = self.systemctl(&["restart", SERVICE_NAME])?;
        if !out.status.success() {
            anyhow::bail!(
                "systemctl restart failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Ok(())
    }

    fn status(&self) -> Result<ServiceStatus> {
        let out = self.systemctl(&["is-active", SERVICE_NAME])?;
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
