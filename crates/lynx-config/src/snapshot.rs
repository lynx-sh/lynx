use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use lynx_core::error::{LynxError, Result};

const MAX_SNAPSHOTS: usize = 10;

/// Directory where snapshots are stored: `~/.config/lynx/snapshots/`.
pub fn snapshots_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".config/lynx/snapshots")
}

/// Create a snapshot of the config dir, returning the snapshot path.
/// Trims oldest snapshots to keep at most MAX_SNAPSHOTS.
pub fn create(config_dir: &Path, label: &str) -> Result<PathBuf> {
    create_in(&snapshots_dir(), config_dir, label)
}

/// List snapshots sorted newest-first as (timestamp_label, path).
pub fn list() -> Result<Vec<(String, PathBuf)>> {
    list_in(&snapshots_dir())
}

fn create_in(snaps_dir: &Path, config_dir: &Path, label: &str) -> Result<PathBuf> {
    std::fs::create_dir_all(snaps_dir).map_err(LynxError::Io)?;

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let safe_label: String = label
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
        .collect();

    let snap_dir = snaps_dir.join(format!("{ts}_{safe_label}"));
    std::fs::create_dir_all(&snap_dir).map_err(LynxError::Io)?;

    // Copy config files (non-recursive: direct children only).
    copy_config_files(config_dir, &snap_dir)?;

    // Trim to MAX_SNAPSHOTS.
    trim_snapshots(snaps_dir)?;

    Ok(snap_dir)
}

fn list_in(snaps_dir: &Path) -> Result<Vec<(String, PathBuf)>> {
    if !snaps_dir.exists() {
        return Ok(vec![]);
    }

    let mut entries: Vec<(String, PathBuf)> = std::fs::read_dir(snaps_dir)
        .map_err(LynxError::Io)?
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            (name, e.path())
        })
        .collect();

    // Sort newest-first (timestamps are prefixes).
    entries.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(entries)
}

/// Restore a snapshot to the config dir.
pub fn restore(snap_dir: &Path, config_dir: &Path) -> Result<()> {
    if !snap_dir.exists() {
        return Err(LynxError::Config(format!(
            "snapshot not found: {snap_dir:?}"
        )));
    }
    for entry in std::fs::read_dir(snap_dir).map_err(LynxError::Io)?.flatten() {
        let src = entry.path();
        let dest = config_dir.join(entry.file_name());
        std::fs::copy(&src, &dest).map_err(LynxError::Io)?;
    }
    Ok(())
}

fn copy_config_files(src: &Path, dest: &Path) -> Result<()> {
    for entry in std::fs::read_dir(src).map_err(LynxError::Io)?.flatten() {
        let path = entry.path();
        // Only copy files (not subdirs — snapshots/ itself lives here).
        if path.is_file() {
            let dest_file = dest.join(entry.file_name());
            std::fs::copy(&path, &dest_file).map_err(LynxError::Io)?;
        }
    }
    Ok(())
}

fn trim_snapshots(dir: &Path) -> Result<()> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(LynxError::Io)?
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.path())
        .collect();

    if entries.len() <= MAX_SNAPSHOTS {
        return Ok(());
    }

    // Sort oldest-first, remove excess.
    entries.sort();
    let to_remove = entries.len() - MAX_SNAPSHOTS;
    for path in entries.iter().take(to_remove) {
        std::fs::remove_dir_all(path).map_err(LynxError::Io)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Returns (config_dir, snaps_dir) — both isolated temp dirs.
    fn setup() -> (tempfile::TempDir, tempfile::TempDir) {
        let config = tempfile::tempdir().unwrap();
        let snaps = tempfile::tempdir().unwrap();
        fs::write(config.path().join("config.toml"), "active_theme = \"default\"").unwrap();
        (config, snaps)
    }

    #[test]
    fn create_snapshot_copies_files() {
        let (config, snaps) = setup();
        let snap = create_in(snaps.path(), config.path(), "test-label").unwrap();
        assert!(snap.join("config.toml").exists());
    }

    #[test]
    fn restore_copies_files_back() {
        let (config, snaps) = setup();
        let snap = create_in(snaps.path(), config.path(), "restore-test").unwrap();

        // Corrupt the config.
        fs::write(config.path().join("config.toml"), "broken = true").unwrap();

        restore(&snap, config.path()).unwrap();
        let content = fs::read_to_string(config.path().join("config.toml")).unwrap();
        assert!(content.contains("default"));
    }

    #[test]
    fn list_returns_newest_first() {
        let (config, snaps) = setup();
        create_in(snaps.path(), config.path(), "first").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        create_in(snaps.path(), config.path(), "second").unwrap();

        let entries = list_in(snaps.path()).unwrap();
        assert_eq!(entries.len(), 2);
        for window in entries.windows(2) {
            assert!(window[0].0 >= window[1].0, "list not newest-first");
        }
    }

    #[test]
    fn trim_keeps_max_snapshots() {
        let (config, snaps) = setup();
        // Create MAX_SNAPSHOTS + 2 snapshots with small delays.
        for i in 0..=(MAX_SNAPSHOTS + 1) {
            create_in(snaps.path(), config.path(), &format!("s{i}")).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let entries = list_in(snaps.path()).unwrap();
        assert!(entries.len() <= MAX_SNAPSHOTS, "too many snapshots: {}", entries.len());
    }
}
