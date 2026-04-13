use crate::{ServiceBackend, ServiceStatus};
use anyhow::{Context, Result};
use lynx_core::{brand, env_vars, paths};
use std::path::PathBuf;

pub struct LaunchdBackend {
    plist_path: PathBuf,
    binary_path: PathBuf,
}

impl LaunchdBackend {
    pub fn new() -> Self {
        let home = std::env::var(env_vars::HOME).unwrap_or_default();
        Self {
            plist_path: PathBuf::from(&home)
                .join("Library/LaunchAgents")
                .join(format!("{}.plist", brand::LAUNCHD_LABEL)),
            binary_path: Self::find_binary(),
        }
    }

    fn find_binary() -> PathBuf {
        // Prefer $LYNX_DAEMON_BIN override (used in tests / custom installs).
        if let Ok(p) = std::env::var(env_vars::LYNX_DAEMON_BIN) {
            return PathBuf::from(p);
        }
        // Fall back to `which lynx-daemon`.
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

    fn plist_content(&self) -> String {
        let log_dir = paths::logs_dir().to_string_lossy().into_owned();

        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{label}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{bin}</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>StandardOutPath</key>
  <string>{log_dir}/daemon.log</string>
  <key>StandardErrorPath</key>
  <string>{log_dir}/daemon.err</string>
  <key>EnvironmentVariables</key>
  <dict>
    <key>HOME</key>
    <string>{home}</string>
  </dict>
</dict>
</plist>
"#,
            label = brand::LAUNCHD_LABEL,
            bin = self.binary_path.display(),
            home = std::env::var(env_vars::HOME).unwrap_or_default(),
        )
    }
}

impl Default for LaunchdBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceBackend for LaunchdBackend {
    fn install(&self) -> Result<()> {
        // Ensure LaunchAgents directory exists.
        if let Some(parent) = self.plist_path.parent() {
            std::fs::create_dir_all(parent).context("failed to create LaunchAgents directory")?;
        }

        std::fs::write(&self.plist_path, self.plist_content())
            .context("failed to write launchd plist")?;

        // Load (or reload) the agent.
        let _ = std::process::Command::new("launchctl")
            .args(["unload", &self.plist_path.to_string_lossy()])
            .output();

        let out = std::process::Command::new("launchctl")
            .args(["load", &self.plist_path.to_string_lossy()])
            .output()
            .context("launchctl load failed")?;

        if !out.status.success() {
            return Err(lynx_core::error::LynxError::Daemon(format!(
                "launchctl load failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ))
            .into());
        }

        Ok(())
    }

    fn uninstall(&self) -> Result<()> {
        let _ = std::process::Command::new("launchctl")
            .args(["unload", &self.plist_path.to_string_lossy()])
            .output();

        if self.plist_path.exists() {
            std::fs::remove_file(&self.plist_path).context("failed to remove plist")?;
        }

        Ok(())
    }

    fn start(&self) -> Result<()> {
        let out = std::process::Command::new("launchctl")
            .args(["start", brand::LAUNCHD_LABEL])
            .output()
            .context("launchctl start failed")?;

        if !out.status.success() {
            return Err(lynx_core::error::LynxError::Daemon(format!(
                "launchctl start failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ))
            .into());
        }
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        let out = std::process::Command::new("launchctl")
            .args(["stop", brand::LAUNCHD_LABEL])
            .output()
            .context("launchctl stop failed")?;

        if !out.status.success() {
            return Err(lynx_core::error::LynxError::Daemon(format!(
                "launchctl stop failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ))
            .into());
        }
        Ok(())
    }

    fn restart(&self) -> Result<()> {
        self.stop().ok(); // ignore stop errors — may already be stopped
        self.start()
    }

    fn status(&self) -> Result<ServiceStatus> {
        let out = std::process::Command::new("launchctl")
            .args(["list", brand::LAUNCHD_LABEL])
            .output()
            .context("launchctl list failed")?;

        if !out.status.success() {
            return Ok(ServiceStatus::Stopped);
        }

        let text = String::from_utf8_lossy(&out.stdout);
        // launchctl list output contains "PID" key if running.
        if text.contains("\"PID\"")
            || text.lines().any(|l| {
                let parts: Vec<&str> = l.split_whitespace().collect();
                !parts.is_empty() && parts[0] != "-" && parts[0].parse::<u32>().is_ok()
            })
        {
            Ok(ServiceStatus::Running)
        } else {
            Ok(ServiceStatus::Stopped)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_contains_label() {
        let backend = LaunchdBackend::new();
        let content = backend.plist_content();
        assert!(content.contains(brand::LAUNCHD_LABEL));
    }

    #[test]
    fn plist_contains_keep_alive() {
        let backend = LaunchdBackend::new();
        let content = backend.plist_content();
        assert!(content.contains("KeepAlive"));
    }

    #[test]
    fn plist_xml_structure() {
        let backend = LaunchdBackend::new();
        let content = backend.plist_content();
        assert!(content.starts_with("<?xml"));
        assert!(content.contains("<plist version=\"1.0\">"));
    }
}
