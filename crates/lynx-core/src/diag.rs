/// Diagnostic log writer.
///
/// Appends structured entries to `$LYNX_DIR/logs/lx-diag.log`.
/// Used to surface errors from background operations (init, plugin load)
/// that cannot print to stderr without corrupting the terminal.
///
/// The log is append-only and never panics — write failures are silently
/// ignored so that a missing log dir never breaks shell startup.
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

/// Append an entry to the diagnostic log.
pub fn log(level: &str, source: &str, msg: &str) {
    let log_path = crate::paths::lynx_dir().join("logs").join("lx-diag.log");
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        let ts = unix_ts();
        let _ = writeln!(f, "{ts} [{level}] {source}: {msg}");
    }
}

pub fn warn(source: &str, msg: &str) {
    log("WARN", source, msg);
}

pub fn error(source: &str, msg: &str) {
    log("ERROR", source, msg);
}

/// Path to the diagnostic log file.
pub fn log_path() -> std::path::PathBuf {
    crate::paths::lynx_dir().join("logs").join("lx-diag.log")
}

/// Read the last `n` lines of the diagnostic log.
/// Returns an empty vec if the log doesn't exist.
pub fn tail(n: usize) -> Vec<String> {
    let path = log_path();
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    content
        .lines()
        .rev()
        .take(n)
        .map(|l| l.to_string())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

/// Clear the diagnostic log.
pub fn clear() -> std::io::Result<()> {
    let path = log_path();
    if path.exists() {
        std::fs::write(&path, "")?;
    }
    Ok(())
}

fn unix_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn with_temp_lynx_dir() -> TempDir {
        let dir = TempDir::new().expect("tempdir");
        std::env::set_var("LYNX_DIR", dir.path());
        dir
    }

    #[test]
    fn log_creates_file_and_appends() {
        let _dir = with_temp_lynx_dir();

        log("INFO", "test", "hello world");
        log("WARN", "test", "second line");

        let lines = tail(10);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("[INFO]"));
        assert!(lines[0].contains("hello world"));
        assert!(lines[1].contains("[WARN]"));
    }

    #[test]
    fn tail_returns_empty_when_no_log() {
        let dir = TempDir::new().expect("tempdir");
        std::env::set_var("LYNX_DIR", dir.path());
        // Don't create the log file — tail must handle missing file gracefully
        let lines = tail(10);
        assert!(lines.is_empty());
    }

    #[test]
    fn clear_empties_log() {
        let _dir = with_temp_lynx_dir();

        log("INFO", "test", "entry");
        clear().unwrap();
        let lines = tail(10);
        assert!(lines.is_empty());
    }
}
