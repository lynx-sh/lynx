use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use tracing::{debug, info};

use crate::index::{
    get_index, load_lock_from, lock_path, plugins_install_dir, save_lock_to, validate_index,
};
use crate::schema::LockEntry;

/// Options for a fetch operation.
#[derive(Debug, Default)]
pub struct FetchOptions {
    /// Specific version to install. None = use latest.
    pub version: Option<String>,
    /// Overwrite an existing installation.
    pub force: bool,
    /// Refresh the registry index before resolving.
    pub refresh_index: bool,
    /// Override the registry index URL.
    pub index_url: Option<String>,
}

/// Download, verify, and install a plugin from the registry.
///
/// Returns the path to the installed plugin directory.
pub fn fetch_plugin(name: &str, opts: &FetchOptions) -> Result<PathBuf> {
    // 1. Resolve from index — validate structure before trusting checksum data.
    let idx = get_index(opts.refresh_index, opts.index_url.as_deref())?;
    validate_index(&idx).context("registry index failed validation — run `lx plugin refresh`")?;
    let entry = idx
        .find(name)
        .with_context(|| format!("plugin '{name}' not found in registry"))?;

    let version_str = opts.version.as_deref();
    let pv = entry.resolve_version(version_str).with_context(|| {
        format!(
            "version '{}' not found for plugin '{name}'",
            version_str.unwrap_or("latest")
        )
    })?;

    // 2. Check if already installed.
    let install_dir = plugins_install_dir().join(name);
    if install_dir.exists() && !opts.force {
        bail!(
            "plugin '{name}' is already installed at {} — use --force to reinstall",
            install_dir.display()
        );
    }

    // 3. Download to a temp dir.
    let tmp_dir = tempfile::tempdir().context("failed to create temp dir for download")?;
    let archive_path = tmp_dir.path().join(format!("{name}.tar.gz"));

    info!("downloading {} v{} from {}", name, pv.version, pv.url);
    download_file(&pv.url, &archive_path)
        .with_context(|| format!("failed to download {} from {}", name, pv.url))?;

    // 4. Verify checksum — abort and clean up if mismatch.
    let actual = sha256_hex(&archive_path).context("failed to compute checksum")?;
    if actual != pv.checksum_sha256 {
        // tmp_dir auto-cleans on drop.
        bail!(
            "checksum mismatch for '{name}' v{}: expected {}, got {actual}",
            pv.version,
            pv.checksum_sha256
        );
    }
    debug!("checksum verified for {name} v{}", pv.version);

    // 5. Extract to install dir.
    if install_dir.exists() {
        std::fs::remove_dir_all(&install_dir).context("failed to remove existing plugin dir")?;
    }
    std::fs::create_dir_all(&install_dir).context("failed to create plugin install dir")?;

    extract_tar_gz(&archive_path, &install_dir).with_context(|| {
        // Clean up partial extract on failure.
        let _ = std::fs::remove_dir_all(&install_dir);
        format!("failed to extract archive for '{name}'")
    })?;

    // 6. Validate plugin.toml in the extracted dir.
    validate_plugin_dir(&install_dir, name)?;

    // 7. Update lynx.lock.
    let lock_file = lock_path();
    let mut lock = load_lock_from(&lock_file).unwrap_or_default();
    let installed_checksum = checksum_plugin_dir(&install_dir)?;
    lock.upsert(LockEntry {
        name: name.to_string(),
        version: pv.version.clone(),
        checksum_sha256: pv.checksum_sha256.clone(),
        installed_checksum_sha256: Some(installed_checksum),
        url: pv.url.clone(),
        source: "registry".to_string(),
    });
    save_lock_to(&lock, &lock_file).context("failed to update lynx.lock")?;

    info!(
        "installed '{name}' v{} to {}",
        pv.version,
        install_dir.display()
    );
    Ok(install_dir)
}

/// Check if a newer version of an installed plugin is available in the registry.
/// Returns `Some((current_version, new_version))` if an upgrade is available.
pub fn check_for_update(
    name: &str,
    refresh: bool,
    index_url: Option<&str>,
) -> Result<Option<(String, String)>> {
    let lock_file = lock_path();
    let lock = load_lock_from(&lock_file).unwrap_or_default();
    let locked = match lock.find(name) {
        Some(e) if e.source == "registry" => e,
        _ => return Ok(None), // locally installed — skip
    };

    let idx = get_index(refresh, index_url)?;
    let entry = match idx.find(name) {
        Some(e) => e,
        None => return Ok(None),
    };

    if entry.latest_version != locked.version {
        Ok(Some((locked.version.clone(), entry.latest_version.clone())))
    } else {
        Ok(None)
    }
}

/// Update a single plugin to its latest registry version.
pub fn update_plugin(name: &str, index_url: Option<&str>) -> Result<PathBuf> {
    fetch_plugin(
        name,
        &FetchOptions {
            force: true,
            refresh_index: true,
            index_url: index_url.map(str::to_string),
            ..Default::default()
        },
    )
}

// ── internals ────────────────────────────────────────────────────────────────

fn download_file(url: &str, dest: &Path) -> Result<()> {
    let resp = ureq::get(url)
        .call()
        .with_context(|| format!("GET {url}"))?;
    if resp.status() >= 400 {
        bail!("server returned status {} for {url}", resp.status());
    }
    let mut reader = resp.into_reader();
    let mut file =
        std::fs::File::create(dest).with_context(|| format!("create {}", dest.display()))?;
    std::io::copy(&mut reader, &mut file).context("write download")?;
    Ok(())
}

fn sha256_hex(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf).context("read for checksum")?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn extract_tar_gz(archive: &Path, dest: &Path) -> Result<()> {
    let file = std::fs::File::open(archive)
        .with_context(|| format!("open archive {}", archive.display()))?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut tar = tar::Archive::new(gz);
    // Strip the top-level directory from the archive (common convention).
    for entry in tar.entries().context("read tar entries")? {
        let mut entry = entry.context("read tar entry")?;
        let entry_path = entry.path().context("entry path")?;
        // Skip the top-level component (e.g. "git-1.0.0/").
        let stripped = entry_path.components().skip(1).collect::<PathBuf>();
        if stripped.as_os_str().is_empty() {
            continue;
        }
        let out_path = dest.join(&stripped);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).context("create extract dir")?;
        }
        entry
            .unpack(&out_path)
            .with_context(|| format!("unpack {}", stripped.display()))?;
    }
    Ok(())
}

fn validate_plugin_dir(dir: &Path, name: &str) -> Result<()> {
    let manifest_path = dir.join(lynx_core::brand::PLUGIN_MANIFEST);
    if !manifest_path.exists() {
        bail!(
            "extracted plugin '{name}' is missing plugin.toml at {}",
            manifest_path.display()
        );
    }
    let content = std::fs::read_to_string(&manifest_path).context("read extracted plugin.toml")?;
    let manifest: lynx_manifest::schema::PluginManifest = toml::from_str(&content)
        .with_context(|| format!("plugin '{name}' has invalid plugin.toml after extraction"))?;
    if manifest.plugin.name != name {
        bail!(
            "plugin '{name}' plugin.toml declares name '{}' — must match",
            manifest.plugin.name
        );
    }
    Ok(())
}

// ── Public checksum utility (used by CLI and registry-spec docs) ───────────

/// Compute the SHA-256 hex digest of a file.
/// This is the same function used internally for verification — same result.
pub fn checksum_file(path: &Path) -> Result<String> {
    sha256_hex(path)
}

/// Compute a deterministic SHA-256 checksum for all files in a directory.
/// Hash input includes relative file paths and file bytes; path order is sorted.
pub fn checksum_plugin_dir(dir: &Path) -> Result<String> {
    if !dir.exists() {
        bail!("plugin directory does not exist: {}", dir.display());
    }

    let mut files = Vec::new();
    collect_files_sorted(dir, dir, &mut files)?;
    files.sort();

    let mut hasher = Sha256::new();
    for rel in files {
        hasher.update(rel.as_bytes());
        hasher.update([0u8]);
        let full = dir.join(&rel);
        let mut file =
            std::fs::File::open(&full).with_context(|| format!("open {}", full.display()))?;
        let mut buf = [0u8; 65536];
        loop {
            let n = file
                .read(&mut buf)
                .context("read plugin file for checksum")?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
    }

    Ok(hex::encode(hasher.finalize()))
}

fn collect_files_sorted(root: &Path, current: &Path, out: &mut Vec<String>) -> Result<()> {
    let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(current)
        .with_context(|| format!("read dir {}", current.display()))?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()
        .context("read directory entries")?;
    entries.sort();

    for path in entries {
        let meta =
            std::fs::symlink_metadata(&path).with_context(|| format!("stat {}", path.display()))?;
        if meta.is_dir() {
            collect_files_sorted(root, &path, out)?;
        } else if meta.is_file() {
            let rel = path
                .strip_prefix(root)
                .with_context(|| format!("strip prefix for {}", path.display()))?
                .to_string_lossy()
                .replace('\\', "/");
            out.push(rel);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_file_consistent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.bin");
        std::fs::write(&path, b"hello world").unwrap();
        let h1 = checksum_file(&path).unwrap();
        let h2 = checksum_file(&path).unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // SHA-256 hex
    }

    #[test]
    fn checksum_different_content_differs() {
        let dir = tempfile::tempdir().unwrap();
        let p1 = dir.path().join("a");
        let p2 = dir.path().join("b");
        std::fs::write(&p1, b"aaa").unwrap();
        std::fs::write(&p2, b"bbb").unwrap();
        assert_ne!(checksum_file(&p1).unwrap(), checksum_file(&p2).unwrap());
    }

    #[test]
    fn checksum_plugin_dir_changes_when_content_changes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(lynx_core::brand::PLUGIN_MANIFEST), "a").unwrap();
        std::fs::create_dir_all(dir.path().join("shell")).unwrap();
        std::fs::write(dir.path().join("shell/init.zsh"), "b").unwrap();

        let before = checksum_plugin_dir(dir.path()).unwrap();
        std::fs::write(dir.path().join("shell/init.zsh"), "changed").unwrap();
        let after = checksum_plugin_dir(dir.path()).unwrap();
        assert_ne!(before, after);
    }

    #[test]
    fn checksum_plugin_dir_independent_of_file_creation_order() {
        let d1 = tempfile::tempdir().unwrap();
        let d2 = tempfile::tempdir().unwrap();

        std::fs::create_dir_all(d1.path().join("shell")).unwrap();
        std::fs::write(d1.path().join("shell/functions.zsh"), "x").unwrap();
        std::fs::write(d1.path().join(lynx_core::brand::PLUGIN_MANIFEST), "y").unwrap();

        std::fs::write(d2.path().join(lynx_core::brand::PLUGIN_MANIFEST), "y").unwrap();
        std::fs::create_dir_all(d2.path().join("shell")).unwrap();
        std::fs::write(d2.path().join("shell/functions.zsh"), "x").unwrap();

        assert_eq!(
            checksum_plugin_dir(d1.path()).unwrap(),
            checksum_plugin_dir(d2.path()).unwrap()
        );
    }

    #[test]
    fn validate_plugin_dir_rejects_missing_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let result = validate_plugin_dir(dir.path(), "myplugin");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("plugin.toml"));
    }

    #[test]
    fn validate_plugin_dir_rejects_name_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(lynx_core::brand::PLUGIN_MANIFEST),
            r#"
[plugin]
name = "wrongname"
version = "1.0.0"
description = "test"
authors = ["x"]
[load]
lazy = false
hooks = []
[deps]
binaries = []
plugins = []
[exports]
functions = []
aliases = []
[contexts]
disabled_in = []
"#,
        )
        .unwrap();
        let result = validate_plugin_dir(dir.path(), "myplugin");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must match"));
    }

    #[test]
    fn validate_plugin_dir_accepts_valid_manifest() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(lynx_core::brand::PLUGIN_MANIFEST),
            r#"
[plugin]
name = "myplugin"
version = "1.0.0"
description = "test"
authors = ["x"]
[load]
lazy = false
hooks = []
[deps]
binaries = []
plugins = []
[exports]
functions = []
aliases = []
[contexts]
disabled_in = []
"#,
        )
        .unwrap();
        assert!(validate_plugin_dir(dir.path(), "myplugin").is_ok());
    }
}
