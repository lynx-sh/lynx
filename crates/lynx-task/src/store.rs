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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_tasks_file_missing_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("nonexistent.toml");
        let content = read_tasks_file(&path).unwrap();
        assert!(content.is_empty());
    }

    #[test]
    fn read_tasks_file_existing() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("tasks.toml");
        std::fs::write(&path, "hello").unwrap();
        let content = read_tasks_file(&path).unwrap();
        assert_eq!(content, "hello");
    }

    #[test]
    fn parse_tasks_file_empty_string() {
        let file = parse_tasks_file("").unwrap();
        assert!(file.tasks.is_empty());
    }

    #[test]
    fn parse_tasks_file_whitespace_only() {
        let file = parse_tasks_file("   \n  \n  ").unwrap();
        assert!(file.tasks.is_empty());
    }

    #[test]
    fn parse_tasks_file_valid_toml() {
        let content = r#"
            [[task]]
            name = "cleanup"
            run = "rm -rf /tmp/junk"
            cron = "0 3 * * *"
        "#;
        let file = parse_tasks_file(content).unwrap();
        assert_eq!(file.tasks.len(), 1);
        assert_eq!(file.tasks[0].name, "cleanup");
    }

    #[test]
    fn parse_tasks_file_invalid_toml_errors() {
        let result = parse_tasks_file("this is not valid toml {{{");
        assert!(result.is_err());
    }

    #[test]
    fn write_and_read_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sub/tasks.toml");

        let file = TasksFile {
            tasks: vec![crate::schema::Task {
                name: "test".into(),
                description: String::new(),
                run: "echo ok".into(),
                cron: "* * * * *".into(),
                on_fail: crate::schema::OnFail::Log,
                timeout: None,
                log: true,
                enabled: true,
            }],
        };

        write_tasks_file(&path, &file).unwrap();
        assert!(path.exists());

        let content = read_tasks_file(&path).unwrap();
        let back = parse_tasks_file(&content).unwrap();
        assert_eq!(back.tasks.len(), 1);
        assert_eq!(back.tasks[0].name, "test");
    }

    #[test]
    fn read_last_run_no_log_file() {
        let tmp = tempfile::tempdir().unwrap();
        let (time, exit) = read_last_run(tmp.path(), "nonexistent");
        assert_eq!(time, "never");
        assert_eq!(exit, "—");
    }

    #[test]
    fn read_last_run_empty_log() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("empty.jsonl"), "").unwrap();
        let (time, exit) = read_last_run(tmp.path(), "empty");
        assert_eq!(time, "never");
        assert_eq!(exit, "—");
    }

    #[test]
    fn read_last_run_valid_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let entry = serde_json::json!({
            "started_at": 1700000000u64,
            "exit_code": 0,
            "duration_ms": 500
        });
        std::fs::write(
            tmp.path().join("task1.jsonl"),
            format!("{}\n", serde_json::to_string(&entry).unwrap()),
        )
        .unwrap();
        let (time, exit) = read_last_run(tmp.path(), "task1");
        assert!(
            time.contains("s ago"),
            "expected relative time, got: {time}"
        );
        assert_eq!(exit, "0");
    }

    #[test]
    fn read_last_run_timed_out_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let entry = serde_json::json!({
            "started_at": 1700000000u64,
            "timed_out": true
        });
        std::fs::write(
            tmp.path().join("slow.jsonl"),
            format!("{}\n", serde_json::to_string(&entry).unwrap()),
        )
        .unwrap();
        let (_time, exit) = read_last_run(tmp.path(), "slow");
        assert_eq!(exit, "timeout");
    }

    #[test]
    fn read_last_run_invalid_json_returns_question() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("bad.jsonl"), "not json\n").unwrap();
        let (time, exit) = read_last_run(tmp.path(), "bad");
        assert_eq!(time, "?");
        assert_eq!(exit, "?");
    }

    #[test]
    fn read_last_run_uses_last_line() {
        let tmp = tempfile::tempdir().unwrap();
        let line1 = serde_json::json!({"started_at": 1700000000u64, "exit_code": 1});
        let line2 = serde_json::json!({"started_at": 1700000100u64, "exit_code": 0});
        let content = format!(
            "{}\n{}\n",
            serde_json::to_string(&line1).unwrap(),
            serde_json::to_string(&line2).unwrap()
        );
        std::fs::write(tmp.path().join("multi.jsonl"), content).unwrap();
        let (_time, exit) = read_last_run(tmp.path(), "multi");
        assert_eq!(exit, "0", "should use the last line");
    }
}
