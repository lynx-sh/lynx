//! Package manager detection and tool installation.
//!
//! Detects the user's package manager (brew, apt, dnf, pacman, cargo),
//! resolves the correct install command from a RegistryEntry's install table,
//! and shells out with user confirmation.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use lynx_core::error::LynxError;
use tracing::info;

use crate::schema::{InstallMethods, RegistryEntry};

// ── Package manager detection ───────────────────────────────────────────────

/// Detected package manager on the system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageManager {
    Brew,
    Apt,
    Dnf,
    Pacman,
    Cargo,
    None,
}

impl PackageManager {
    pub fn label(&self) -> &'static str {
        match self {
            PackageManager::Brew => "brew",
            PackageManager::Apt => "apt",
            PackageManager::Dnf => "dnf",
            PackageManager::Pacman => "pacman",
            PackageManager::Cargo => "cargo",
            PackageManager::None => "none",
        }
    }
}

/// Detect the available package manager by checking PATH.
/// Checks in priority order: brew → apt → dnf → pacman → cargo.
pub fn detect_package_manager() -> PackageManager {
    let checks: &[(&str, PackageManager)] = &[
        ("brew", PackageManager::Brew),
        ("apt", PackageManager::Apt),
        ("dnf", PackageManager::Dnf),
        ("pacman", PackageManager::Pacman),
        ("cargo", PackageManager::Cargo),
    ];
    for (bin, pm) in checks {
        if lynx_core::paths::find_binary(bin).is_some() {
            return pm.clone();
        }
    }
    PackageManager::None
}

// ── Install command resolution ──────────────────────────────────────────────

/// Resolve the install command for a tool given the detected package manager.
/// Returns (command, args) — e.g. ("brew", ["install", "eza"]).
pub fn resolve_install_command(
    install: &InstallMethods,
    pm: &PackageManager,
) -> Option<(String, Vec<String>)> {
    match pm {
        PackageManager::Brew => install.brew.as_ref().map(|pkg| {
            ("brew".into(), vec!["install".into(), pkg.clone()])
        }),
        PackageManager::Apt => install.apt.as_ref().map(|pkg| {
            ("sudo".into(), vec!["apt".into(), "install".into(), "-y".into(), pkg.clone()])
        }),
        PackageManager::Dnf => install.dnf.as_ref().map(|pkg| {
            ("sudo".into(), vec!["dnf".into(), "install".into(), "-y".into(), pkg.clone()])
        }),
        PackageManager::Pacman => install.pacman.as_ref().map(|pkg| {
            ("sudo".into(), vec!["pacman".into(), "-S".into(), "--noconfirm".into(), pkg.clone()])
        }),
        PackageManager::Cargo => install.cargo.as_ref().map(|pkg| {
            ("cargo".into(), vec!["install".into(), pkg.clone()])
        }),
        PackageManager::None => None,
    }
}

/// Build the install command for a URL-based install (download to ~/.local/bin/).
pub fn resolve_url_install(url: &str, name: &str) -> (PathBuf, String) {
    let dest = lynx_core::paths::bin_dir().join(name);
    (dest, url.to_string())
}

// ── Tool installation ───────────────────────────────────────────────────────

/// Install a tool using the system package manager.
/// Returns the binary name that was installed.
pub fn install_tool_via_pm(entry: &RegistryEntry) -> Result<String> {
    let install = entry
        .install
        .as_ref()
        .ok_or_else(|| LynxError::Registry(format!("package '{}' has no install methods defined", entry.name)))?;

    let pm = detect_package_manager();

    // Try package manager first, then fall back to URL.
    if let Some((cmd, args)) = resolve_install_command(install, &pm) {
        info!("installing '{}' via {} {}", entry.name, cmd, args.join(" "));
        let status = Command::new(&cmd)
            .args(&args)
            .status()
            .with_context(|| format!("failed to run: {} {}", cmd, args.join(" ")))?;

        if !status.success() {
            return Err(LynxError::Registry(format!(
                "{} {} failed with exit code {}",
                cmd, args.join(" "), status.code().unwrap_or(-1)
            )).into());
        }

        // Verify the binary exists after install.
        let binary_name = entry.replaces.as_deref().unwrap_or(&entry.name);
        if lynx_core::paths::find_binary(binary_name).is_none() {
            // Some tools install under a different name — check the entry name too.
            if lynx_core::paths::find_binary(&entry.name).is_none() {
                eprintln!(
                    "warning: '{}' installed but binary '{}' not found on PATH",
                    entry.name, binary_name
                );
            }
        }

        info!("installed '{}' via {}", entry.name, pm.label());
        return Ok(entry.name.clone());
    }

    // Fall back to URL download.
    if let Some(url) = &install.url {
        return install_tool_via_url(url, &entry.name);
    }

    Err(LynxError::Registry(format!(
        "no install method available for '{}' on {} — try installing manually",
        entry.name, pm.label()
    )).into())
}

/// Download a tool binary from a URL to ~/.local/bin/.
fn install_tool_via_url(url: &str, name: &str) -> Result<String> {
    let (dest, _url) = resolve_url_install(url, name);

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).context("failed to create bin dir")?;
    }

    info!("downloading '{}' from {}", name, url);
    let resp = ureq::get(url)
        .call()
        .with_context(|| format!("GET {url}"))?;
    if resp.status() >= 400 {
        return Err(LynxError::Registry(format!("server returned status {} for {url}", resp.status())).into());
    }

    let mut reader = resp.into_reader();
    let mut file =
        std::fs::File::create(&dest).with_context(|| format!("create {}", dest.display()))?;
    std::io::copy(&mut reader, &mut file).context("write download")?;

    // Make executable on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&dest)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&dest, perms)?;
    }

    info!("installed '{}' to {}", name, dest.display());
    Ok(name.to_string())
}

// ── Theme and intro installation ────────────────────────────────────────────

/// Download a theme TOML from a URL to ~/.config/lynx/themes/<name>.toml.
/// Validates the theme before writing (fails if invalid).
pub fn install_theme(name: &str, url: &str, force: bool) -> Result<PathBuf> {
    let dest = lynx_core::paths::themes_dir().join(format!("{name}.toml"));
    if dest.exists() && !force {
        return Err(LynxError::AlreadyInstalled(format!("theme '{name}'")).into());
    }

    let body = download_text(url)
        .with_context(|| format!("failed to download theme '{name}' from {url}"))?;

    // Validate before writing.
    if let Err(e) = toml::from_str::<toml::Value>(&body) {
        return Err(LynxError::Theme(format!("downloaded theme '{name}' is invalid TOML: {e}")).into());
    }

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).context("create themes dir")?;
    }
    std::fs::write(&dest, &body)
        .with_context(|| format!("write theme to {}", dest.display()))?;

    info!("installed theme '{}' to {}", name, dest.display());
    Ok(dest)
}

/// Download an intro TOML from a URL to ~/.config/lynx/intros/<name>.toml.
pub fn install_intro(name: &str, url: &str, force: bool) -> Result<PathBuf> {
    let intros_dir = lynx_core::paths::lynx_dir().join("intros");
    let dest = intros_dir.join(format!("{name}.toml"));
    if dest.exists() && !force {
        return Err(LynxError::AlreadyInstalled(format!("intro '{name}'")).into());
    }

    let body = download_text(url)
        .with_context(|| format!("failed to download intro '{name}' from {url}"))?;

    if let Err(e) = toml::from_str::<toml::Value>(&body) {
        return Err(LynxError::Theme(format!("downloaded intro '{name}' is invalid TOML: {e}")).into());
    }

    std::fs::create_dir_all(&intros_dir).context("create intros dir")?;
    std::fs::write(&dest, &body)
        .with_context(|| format!("write intro to {}", dest.display()))?;

    info!("installed intro '{}' to {}", name, dest.display());
    Ok(dest)
}

fn download_text(url: &str) -> Result<String> {
    let resp = ureq::get(url)
        .call()
        .with_context(|| format!("GET {url}"))?;
    if resp.status() >= 400 {
        return Err(LynxError::Registry(format!("server returned status {} for {url}", resp.status())).into());
    }
    resp.into_string().context("read response body")
}

// ── Uninstall ───────────────────────────────────────────────────────────────

/// Remove the auto-generated Lynx plugin for a tool.
/// Does NOT remove the system binary — prints instructions instead.
pub fn uninstall_tool(name: &str, plugins_dir: &Path) -> Result<()> {
    let plugin_dir = plugins_dir.join(name);
    if plugin_dir.exists() {
        std::fs::remove_dir_all(&plugin_dir)
            .with_context(|| format!("failed to remove plugin dir {}", plugin_dir.display()))?;
        println!("removed Lynx plugin for '{name}'");
    }

    let pm = detect_package_manager();
    let uninstall_hint = match pm {
        PackageManager::Brew => format!("brew uninstall {name}"),
        PackageManager::Apt => format!("sudo apt remove {name}"),
        PackageManager::Dnf => format!("sudo dnf remove {name}"),
        PackageManager::Pacman => format!("sudo pacman -R {name}"),
        PackageManager::Cargo => format!("cargo uninstall {name}"),
        PackageManager::None => format!("manually remove the '{name}' binary"),
    };
    println!("system binary preserved — to remove it: {uninstall_hint}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::InstallMethods;

    #[test]
    fn detect_package_manager_finds_something() {
        // On any dev machine, at least cargo should be available.
        let pm = detect_package_manager();
        assert_ne!(pm, PackageManager::None, "expected at least one PM on PATH");
    }

    #[test]
    fn resolve_brew_install() {
        let methods = InstallMethods {
            brew: Some("eza".into()),
            ..Default::default()
        };
        let (cmd, args) = resolve_install_command(&methods, &PackageManager::Brew).unwrap();
        assert_eq!(cmd, "brew");
        assert_eq!(args, vec!["install", "eza"]);
    }

    #[test]
    fn resolve_apt_install() {
        let methods = InstallMethods {
            apt: Some("fd-find".into()),
            ..Default::default()
        };
        let (cmd, args) = resolve_install_command(&methods, &PackageManager::Apt).unwrap();
        assert_eq!(cmd, "sudo");
        assert!(args.contains(&"fd-find".to_string()));
    }

    #[test]
    fn resolve_cargo_install() {
        let methods = InstallMethods {
            cargo: Some("ripgrep".into()),
            ..Default::default()
        };
        let (cmd, args) = resolve_install_command(&methods, &PackageManager::Cargo).unwrap();
        assert_eq!(cmd, "cargo");
        assert_eq!(args, vec!["install", "ripgrep"]);
    }

    #[test]
    fn resolve_returns_none_when_no_method() {
        let methods = InstallMethods {
            apt: Some("eza".into()),
            ..Default::default()
        };
        // Brew has no entry — should return None.
        assert!(resolve_install_command(&methods, &PackageManager::Brew).is_none());
    }

    #[test]
    fn resolve_url_install_path() {
        let (dest, url) = resolve_url_install("https://example.com/tool", "mytool");
        assert!(dest.to_string_lossy().ends_with("mytool"));
        assert_eq!(url, "https://example.com/tool");
    }

    #[test]
    fn uninstall_tool_removes_plugin_dir() {
        let dir = tempfile::tempdir().unwrap();
        let plugin_dir = dir.path().join("eza");
        std::fs::create_dir_all(plugin_dir.join("shell")).unwrap();
        std::fs::write(plugin_dir.join("plugin.toml"), "x").unwrap();

        uninstall_tool("eza", dir.path()).unwrap();
        assert!(!plugin_dir.exists());
    }

    #[test]
    fn uninstall_tool_handles_missing_dir() {
        let dir = tempfile::tempdir().unwrap();
        // No plugin dir exists — should not error.
        uninstall_tool("nonexistent", dir.path()).unwrap();
    }
}
