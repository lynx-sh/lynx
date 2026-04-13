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
