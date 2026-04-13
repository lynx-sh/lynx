use serde::{Deserialize, Serialize};
use std::time::Duration;

/// What to do when a task exits non-zero.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OnFail {
    #[default]
    Log,
    Notify,
    Ignore,
}

/// A single task entry from `[[task]]` in tasks.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique task name (required).
    pub name: String,

    /// Human description (optional).
    #[serde(default)]
    pub description: String,

    /// Shell command to run (required).
    pub run: String,

    /// Cron expression (required).  5-field standard: min hr dom mon dow.
    pub cron: String,

    /// What to do on non-zero exit (default: log).
    #[serde(default)]
    pub on_fail: OnFail,

    /// Timeout as a human duration string, e.g. "60s", "5m", "1h" (optional).
    #[serde(default)]
    pub timeout: Option<String>,

    /// Write stdout/stderr to the task log file (default: true).
    #[serde(default = "default_true")]
    pub log: bool,

    /// Whether the task is active (default: true).
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_fail_default_is_log() {
        let on_fail = OnFail::default();
        assert_eq!(on_fail, OnFail::Log);
    }

    #[test]
    fn on_fail_serde_roundtrip() {
        let values = vec![OnFail::Log, OnFail::Notify, OnFail::Ignore];
        for v in values {
            let json = serde_json::to_string(&v).unwrap();
            let back: OnFail = serde_json::from_str(&json).unwrap();
            assert_eq!(v, back);
        }
    }

    #[test]
    fn task_deserialize_minimal() {
        let toml = r#"
            name = "test"
            run = "echo hi"
            cron = "* * * * *"
        "#;
        let task: Task = toml::from_str(toml).unwrap();
        assert_eq!(task.name, "test");
        assert_eq!(task.run, "echo hi");
        assert_eq!(task.on_fail, OnFail::Log);
        assert!(task.enabled);
        assert!(task.log);
        assert!(task.timeout.is_none());
        assert!(task.description.is_empty());
    }

    #[test]
    fn task_deserialize_full() {
        let toml = r#"
            name = "backup"
            description = "daily backup"
            run = "rsync -a ~/docs /backup/"
            cron = "0 2 * * *"
            on_fail = "notify"
            timeout = "5m"
            log = false
            enabled = false
        "#;
        let task: Task = toml::from_str(toml).unwrap();
        assert_eq!(task.name, "backup");
        assert_eq!(task.description, "daily backup");
        assert_eq!(task.on_fail, OnFail::Notify);
        assert_eq!(task.timeout.as_deref(), Some("5m"));
        assert!(!task.log);
        assert!(!task.enabled);
    }

    #[test]
    fn tasks_file_deserialize_empty() {
        let toml = "";
        let file: TasksFile = toml::from_str(toml).unwrap();
        assert!(file.tasks.is_empty());
    }

    #[test]
    fn tasks_file_deserialize_multiple() {
        let toml = r#"
            [[task]]
            name = "a"
            run = "echo a"
            cron = "* * * * *"

            [[task]]
            name = "b"
            run = "echo b"
            cron = "0 0 * * *"
        "#;
        let file: TasksFile = toml::from_str(toml).unwrap();
        assert_eq!(file.tasks.len(), 2);
        assert_eq!(file.tasks[0].name, "a");
        assert_eq!(file.tasks[1].name, "b");
    }

    #[test]
    fn tasks_file_serialize_roundtrip() {
        let file = TasksFile {
            tasks: vec![Task {
                name: "test".into(),
                description: String::new(),
                run: "echo hi".into(),
                cron: "* * * * *".into(),
                on_fail: OnFail::Log,
                timeout: None,
                log: true,
                enabled: true,
            }],
        };
        let toml_str = toml::to_string_pretty(&file).unwrap();
        let back: TasksFile = toml::from_str(&toml_str).unwrap();
        assert_eq!(back.tasks.len(), 1);
        assert_eq!(back.tasks[0].name, "test");
    }
}

/// Parsed, validated task with a concrete timeout `Duration`.
#[derive(Debug, Clone)]
pub struct ValidatedTask {
    pub task: Task,
    /// `None` means no timeout enforced.
    pub timeout_duration: Option<Duration>,
}

/// Top-level wrapper that matches the TOML file structure.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TasksFile {
    #[serde(rename = "task", default)]
    pub tasks: Vec<Task>,
}
