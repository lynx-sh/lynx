//! Task persistence — read, write, and query tasks.toml and task logs.
//!
//! This is the single place for all task file I/O. lynx-cli and lynx-daemon
//! both call these functions; neither should contain file I/O directly.

use crate::schema::TasksFile;
use anyhow::{Context, Result};
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Load tasks.toml as a raw string. Returns an empty string if the file does not exist.
pub fn read_tasks_file(path: &Path) -> Result<String> {
    if !path.exists() {
        return Ok(String::new());
    }
    std::fs::read_to_string(path).context("failed to read tasks.toml")
}

/// Parse a TOML string into a `TasksFile`. Returns a default (empty) file if the input is blank.
pub fn parse_tasks_file(content: &str) -> Result<TasksFile> {
    if content.trim().is_empty() {
        return Ok(TasksFile::default());
    }
    toml::from_str::<TasksFile>(content).context("tasks.toml parse error")
}

/// Write a `TasksFile` to disk, creating parent directories as needed.
pub fn write_tasks_file(path: &Path, file: &TasksFile) -> Result<()> {
    let parent = path.parent().unwrap_or(path);
    std::fs::create_dir_all(parent).context("failed to create config directory")?;
    let content = toml::to_string_pretty(file).context("failed to serialize tasks.toml")?;
    std::fs::write(path, content).context("failed to write tasks.toml")
}

/// Read the last JSONL entry from a task's log file.
/// Returns `("never", "—")` if no log exists or is empty.
pub fn read_last_run(log_dir: &Path, task_name: &str) -> (String, String) {
    let log_path = log_dir.join(format!("{task_name}.jsonl"));
    let Ok(file) = std::fs::File::open(&log_path) else {
        return ("never".into(), "—".into());
    };

    let reader = BufReader::new(file);
    let last = reader.lines().map_while(Result::ok).last();

    let Some(line) = last else {
        return ("never".into(), "—".into());
    };

    let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) else {
        return ("?".into(), "?".into());
    };

    let ts = val["started_at"].as_u64().unwrap_or(0);
    let exit = match val["exit_code"].as_i64() {
        Some(c) => c.to_string(),
        None if val["timed_out"].as_bool() == Some(true) => "timeout".into(),
        None => "?".into(),
    };

    let time_str = if ts == 0 {
        "?".into()
    } else {
        let elapsed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|now| now.as_secs().saturating_sub(ts))
            .unwrap_or(0);
        format!("{elapsed}s ago")
    };

    (time_str, exit)
}
