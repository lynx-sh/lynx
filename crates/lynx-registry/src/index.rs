use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use tracing::debug;

use lynx_core::brand;

use crate::schema::{LockFile, RegistryIndex};

/// Default registry index URL — official Lynx plugin index.
pub const DEFAULT_INDEX_URL: &str =
    "https://raw.githubusercontent.com/lynx-sh/registry/main/index.toml";

/// Path to the local cached index: `~/.config/lynx/registry/index.toml`.
pub fn index_cache_path() -> PathBuf {
    config_base().join("registry").join("index.toml")
}

/// Path to lynx.lock: `~/.config/lynx/lynx.lock`.
pub fn lock_path() -> PathBuf {
    config_base().join("lynx.lock")
}

/// Path to the installed plugins dir: `~/.local/share/lynx/plugins/`.
pub fn plugins_install_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".local/share/lynx/plugins")
}

fn config_base() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(brand::CONFIG_DIR)
}

// ── Index I/O ─────────────────────────────────────────────────────────────────

/// Parse a registry index from a TOML string.
pub fn parse_index(toml_str: &str) -> Result<RegistryIndex> {
    toml::from_str(toml_str).context("failed to parse registry index TOML")
}

/// Load the locally cached registry index. Returns an error if no cache exists.
pub fn load_cached_index() -> Result<RegistryIndex> {
    load_index_from(&index_cache_path())
}

/// Load a registry index from an explicit path.
pub fn load_index_from(path: &Path) -> Result<RegistryIndex> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read index at {}", path.display()))?;
    parse_index(&content)
}

/// Fetch the registry index from a URL, cache it locally, and return it.
/// Uses a blocking HTTP request — not suitable for the async context; call
/// from a `tokio::task::spawn_blocking` block.
pub fn fetch_and_cache_index(url: &str) -> Result<RegistryIndex> {
    debug!("fetching registry index from {url}");
    let body = reqwest::blocking::get(url)
        .with_context(|| format!("HTTP GET failed for {url}"))?
        .error_for_status()
        .with_context(|| format!("registry index returned error status from {url}"))?
        .text()
        .context("failed to read registry index response body")?;

    let idx = parse_index(&body)?;

    // Cache to disk.
    let cache_path = index_cache_path();
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent).context("failed to create registry cache dir")?;
    }
    std::fs::write(&cache_path, &body).context("failed to write cached index")?;
    debug!("cached registry index at {}", cache_path.display());

    Ok(idx)
}

/// Load the cached index, refreshing from the network if `refresh` is true.
/// Falls back to the cache if the network is unavailable.
pub fn get_index(refresh: bool, url: Option<&str>) -> Result<RegistryIndex> {
    let url = url.unwrap_or(DEFAULT_INDEX_URL);
    if refresh {
        match fetch_and_cache_index(url) {
            Ok(idx) => return Ok(idx),
            Err(e) => {
                eprintln!("warning: could not refresh registry index: {e}");
                eprintln!("         falling back to cached index");
            }
        }
    }
    // Try cache.
    if index_cache_path().exists() {
        return load_cached_index();
    }
    // No cache and no refresh — try fetching as a first-time fallback.
    fetch_and_cache_index(url)
}

// ── Lock file I/O ─────────────────────────────────────────────────────────────

/// Load lynx.lock from disk. Returns an empty LockFile if it doesn't exist.
pub fn load_lock() -> Result<LockFile> {
    load_lock_from(&lock_path())
}

pub fn load_lock_from(path: &Path) -> Result<LockFile> {
    match std::fs::read_to_string(path) {
        Ok(content) => toml::from_str(&content).context("failed to parse lynx.lock"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(LockFile::default()),
        Err(e) => Err(e).context("failed to read lynx.lock"),
    }
}

/// Save lynx.lock to disk.
pub fn save_lock(lock: &LockFile) -> Result<()> {
    save_lock_to(lock, &lock_path())
}

pub fn save_lock_to(lock: &LockFile, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("failed to create lock dir")?;
    }
    let content = toml::to_string_pretty(lock).context("failed to serialize lynx.lock")?;
    std::fs::write(path, content).context("failed to write lynx.lock")?;
    Ok(())
}

// ── Validation ───────────────────────────────────────────────────────────────

/// Validate the index structure: every entry must have at least one version,
/// a non-empty checksum, and a valid latest_version reference.
pub fn validate_index(idx: &RegistryIndex) -> Result<()> {
    for entry in &idx.plugins {
        if entry.versions.is_empty() {
            bail!("plugin '{}' has no versions", entry.name);
        }
        if entry.resolve_version(None).is_none() {
            bail!(
                "plugin '{}' latest_version '{}' not found in versions list",
                entry.name,
                entry.latest_version
            );
        }
        for v in &entry.versions {
            if v.checksum_sha256.is_empty() {
                bail!(
                    "plugin '{}' version '{}' has empty checksum_sha256",
                    entry.name,
                    v.version
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{LockEntry, PluginVersion, RegistryEntry};

    fn sample_index() -> RegistryIndex {
        RegistryIndex {
            plugins: vec![RegistryEntry {
                name: "git".into(),
                description: "Git integration".into(),
                author: "proxikal".into(),
                latest_version: "1.0.0".into(),
                versions: vec![PluginVersion {
                    version: "1.0.0".into(),
                    url: "https://example.com/git-1.0.0.tar.gz".into(),
                    checksum_sha256: "abc123".into(),
                    min_lynx_version: None,
                }],
            }],
        }
    }

    #[test]
    fn parse_index_roundtrip() {
        let idx = sample_index();
        let toml_str = toml::to_string_pretty(&idx).unwrap();
        let parsed = parse_index(&toml_str).unwrap();
        assert_eq!(parsed.plugins.len(), 1);
        assert_eq!(parsed.find("git").unwrap().latest_version, "1.0.0");
    }

    #[test]
    fn validate_valid_index_passes() {
        assert!(validate_index(&sample_index()).is_ok());
    }

    #[test]
    fn validate_empty_versions_fails() {
        let mut idx = sample_index();
        idx.plugins[0].versions.clear();
        assert!(validate_index(&idx).is_err());
    }

    #[test]
    fn validate_missing_latest_ref_fails() {
        let mut idx = sample_index();
        idx.plugins[0].latest_version = "9.9.9".into();
        assert!(validate_index(&idx).is_err());
    }

    #[test]
    fn validate_empty_checksum_fails() {
        let mut idx = sample_index();
        idx.plugins[0].versions[0].checksum_sha256 = "".into();
        assert!(validate_index(&idx).is_err());
    }

    #[test]
    fn lockfile_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lynx.lock");
        let mut lock = LockFile::default();
        lock.upsert(LockEntry {
            name: "git".into(),
            version: "1.0.0".into(),
            checksum_sha256: "abc".into(),
            installed_checksum_sha256: Some("abc".into()),
            url: "https://x.com/git.tar.gz".into(),
            source: "registry".into(),
        });
        save_lock_to(&lock, &path).unwrap();
        let loaded = load_lock_from(&path).unwrap();
        assert_eq!(lock, loaded);
    }

    #[test]
    fn load_lock_missing_file_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let lock = load_lock_from(&dir.path().join("nonexistent.lock")).unwrap();
        assert!(lock.entries.is_empty());
    }
}
