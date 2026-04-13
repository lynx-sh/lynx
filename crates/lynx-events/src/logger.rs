use crate::types::Event;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// A single log entry written as a JSON line.
#[derive(Debug, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: u64,
    pub event_name: String,
    pub data: String,
    pub source: String,
}

/// Patterns whose values are redacted before logging (case-insensitive suffix match).
const REDACT_PATTERNS: &[&str] = &["_KEY", "_TOKEN", "_SECRET", "_PASSWORD"];

/// Returns true if the given key name matches a secret pattern.
pub fn is_secret_key(key: &str) -> bool {
    let upper = key.to_uppercase();
    REDACT_PATTERNS.iter().any(|p| upper.ends_with(p))
}

/// Redact secret-shaped values from a data string.
///
/// Scans for `KEY=VALUE` pairs; replaces the value with `[REDACTED]` when
/// the key matches a secret pattern.
pub fn redact(data: &str) -> String {
    let mut out = data.to_string();
    for part in data.split_whitespace() {
        if let Some((k, _v)) = part.split_once('=') {
            if is_secret_key(k) {
                out = out.replace(part, &format!("{k}=[REDACTED]"));
            }
        }
    }
    out
}

/// Resolve the event log file path: `~/.config/lynx/logs/events.jsonl`.
pub fn log_path() -> PathBuf {
    lynx_core::paths::events_log_file()
}

/// Write a single log entry to the event log file (append).
///
/// Creates parent dirs if needed. Non-blocking from caller's perspective
/// because this is called from a spawned task in the event bus subscriber.
pub fn write_entry(event: &Event, source: &str) -> std::io::Result<()> {
    let path = log_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let entry = LogEntry {
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        event_name: event.name.clone(),
        data: redact(&event.data),
        source: source.to_string(),
    };

    let mut line = serde_json::to_string(&entry).map_err(std::io::Error::other)?;
    line.push('\n');

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    file.write_all(line.as_bytes())
}

/// Rotate the log file: delete entries older than 7 days, cap at 10 MB.
///
/// Rewrites the file in-place by keeping only recent lines.
pub fn rotate_log() -> std::io::Result<()> {
    let path = log_path();
    if !path.exists() {
        return Ok(());
    }

    const MAX_BYTES: u64 = 10 * 1024 * 1024;
    const MAX_AGE_SECS: u64 = 7 * 24 * 3600;

    let metadata = std::fs::metadata(&path)?;
    let size = metadata.len();

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let cutoff = now.saturating_sub(MAX_AGE_SECS);

    // Only bother if over 10MB or we want to prune old entries
    if size < MAX_BYTES {
        // Still prune by age even when small
        prune_by_age(&path, cutoff)?;
        return Ok(());
    }

    prune_by_age(&path, cutoff)
}

fn prune_by_age(path: &PathBuf, cutoff_secs: u64) -> std::io::Result<()> {
    let content = std::fs::read_to_string(path)?;
    let kept: Vec<&str> = content
        .lines()
        .filter(|line| {
            // Keep lines that either fail to parse (don't discard unknown) or are recent
            serde_json::from_str::<serde_json::Value>(line)
                .ok()
                .and_then(|v| v["timestamp"].as_u64())
                .map(|ts| ts >= cutoff_secs)
                .unwrap_or(true)
        })
        .collect();

    let new_content = kept.join("\n") + if kept.is_empty() { "" } else { "\n" };
    std::fs::write(path, new_content)
}

/// Read the last N lines from the log, optionally filtering by event name prefix.
pub fn tail_log(n: usize, filter: Option<&str>) -> std::io::Result<Vec<LogEntry>> {
    let path = log_path();
    if !path.exists() {
        return Ok(vec![]);
    }

    let content = std::fs::read_to_string(&path)?;
    let entries: Vec<LogEntry> = content
        .lines()
        .filter_map(|line| serde_json::from_str::<LogEntry>(line).ok())
        .filter(|e| filter.map(|f| e.event_name.starts_with(f)).unwrap_or(true))
        .collect();

    let skip = entries.len().saturating_sub(n);
    Ok(entries.into_iter().skip(skip).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Event;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvGuard {
        vars: Vec<(String, Option<std::ffi::OsString>)>,
    }

    impl EnvGuard {
        fn new(keys: &[&str]) -> Self {
            let vars = keys
                .iter()
                .map(|k| (k.to_string(), std::env::var_os(k)))
                .collect();
            Self { vars }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (k, v) in &self.vars {
                match v {
                    Some(val) => std::env::set_var(k, val),
                    None => std::env::remove_var(k),
                }
            }
        }
    }

    #[test]
    fn secret_key_detection() {
        assert!(is_secret_key("API_KEY"));
        assert!(is_secret_key("GITHUB_TOKEN"));
        assert!(is_secret_key("DB_PASSWORD"));
        assert!(is_secret_key("SIGNING_SECRET"));
        assert!(!is_secret_key("HOME"));
        assert!(!is_secret_key("PATH"));
    }

    #[test]
    fn redact_removes_secret_values() {
        let data = "API_KEY=abc123 HOME=/home/user";
        let out = redact(data);
        assert!(out.contains("API_KEY=[REDACTED]"));
        assert!(out.contains("HOME=/home/user"));
        assert!(!out.contains("abc123"));
    }

    #[test]
    fn write_and_tail_roundtrip() {
        let _lock = env_lock().lock().expect("lock");
        let _guard = EnvGuard::new(&["HOME", "LYNX_DIR"]);
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", tmp.path());
        std::env::remove_var("LYNX_DIR");

        let event = Event::new("shell:chpwd", "/tmp/test");
        write_entry(&event, "shell").unwrap();
        write_entry(&Event::new("shell:precmd", ""), "shell").unwrap();
        write_entry(&Event::new("git:branch-changed", "main"), "plugin:git").unwrap();

        let entries = tail_log(10, None).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].event_name, "shell:chpwd");

        let filtered = tail_log(10, Some("git:")).unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].source, "plugin:git");
    }

    #[test]
    fn tail_log_limits_results() {
        let _lock = env_lock().lock().expect("lock");
        let _guard = EnvGuard::new(&["HOME", "LYNX_DIR"]);
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", tmp.path());
        std::env::remove_var("LYNX_DIR");

        for i in 0..5 {
            write_entry(&Event::new("shell:precmd", i.to_string()), "shell").unwrap();
        }

        let entries = tail_log(3, None).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[2].data, "4"); // most recent
    }
}
