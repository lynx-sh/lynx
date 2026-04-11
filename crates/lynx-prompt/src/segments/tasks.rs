use crate::segment::{RenderContext, RenderedSegment, Segment};
use lynx_theme::schema::SegmentConfig;
use std::io::BufRead;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const CACHE_KEY: &str = "task_status";
const WARNING_ICON: &str = "⚠";
const TASK_ICON: &str = "󱐌";
/// 24 hours in seconds — window for "recently failed" detection.
const RECENT_FAILURE_SECS: u64 = 86_400;

pub struct TaskStatusSegment;

impl Segment for TaskStatusSegment {
    fn name(&self) -> &'static str {
        "task_status"
    }

    fn cache_key(&self) -> Option<&'static str> {
        Some(CACHE_KEY)
    }

    fn render(&self, _config: &SegmentConfig, ctx: &RenderContext) -> Option<RenderedSegment> {
        // Use cached result if present — refreshed by precmd event at most once per minute.
        if let Some(cached) = ctx.cache.get(CACHE_KEY) {
            let text = cached.as_str()?.to_string();
            if text.is_empty() {
                return None;
            }
            return Some(RenderedSegment::new(text).with_cache_key(CACHE_KEY));
        }

        // No cache — compute inline (first render or cache miss).
        let summary = compute_task_summary()?;
        Some(RenderedSegment::new(summary).with_cache_key(CACHE_KEY))
    }
}

/// Compute the task status summary string.
/// Returns `None` when no tasks are configured or logs are missing.
pub fn compute_task_summary() -> Option<String> {
    let tasks_path = tasks_toml_path();
    if !tasks_path.exists() {
        return None;
    }

    // Count enabled tasks from tasks.toml.
    let content = std::fs::read_to_string(&tasks_path).ok()?;
    let file: toml::Value = toml::from_str(&content).ok()?;
    let tasks = file.get("task")?.as_array()?;
    let total: usize = tasks.len();

    if total == 0 {
        return None;
    }

    let log_dir = task_log_dir();
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Check all task logs for recent failures.
    let mut any_recent_failure = false;

    for task in tasks {
        let name = task.get("name").and_then(|n| n.as_str())?;
        let log_path = log_dir.join(format!("{name}.jsonl"));

        if let Some(entry) = read_last_log_entry(&log_path) {
            let started_at = entry["started_at"].as_u64().unwrap_or(0);
            let age = now_secs.saturating_sub(started_at);
            let timed_out = entry["timed_out"].as_bool().unwrap_or(false);
            let exit_code = entry["exit_code"].as_i64();

            let failed = timed_out || exit_code.map(|c| c != 0).unwrap_or(false);

            if failed && age < RECENT_FAILURE_SECS {
                any_recent_failure = true;
                break;
            }
        }
    }

    let icon = if any_recent_failure {
        format!("{WARNING_ICON} {TASK_ICON}")
    } else {
        TASK_ICON.to_string()
    };

    Some(format!("{icon} {total}"))
}

fn read_last_log_entry(path: &PathBuf) -> Option<serde_json::Value> {
    let file = std::fs::File::open(path).ok()?;
    let reader = std::io::BufReader::new(file);
    let last = reader.lines().map_while(Result::ok).last()?;
    serde_json::from_str(&last).ok()
}

fn tasks_toml_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".config/lynx/tasks.toml")
}

fn task_log_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".config/lynx/logs/tasks")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::RenderContext;
    use lynx_core::types::Context;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn make_ctx_with_cache(key: &str, val: &str) -> RenderContext {
        let mut cache = HashMap::new();
        cache.insert(key.to_string(), serde_json::Value::String(val.to_string()));
        RenderContext {
            cwd: "/".into(),
            shell_context: Context::Interactive,
            last_cmd_ms: None,
            cache,
        }
    }

    #[test]
    fn hidden_when_cache_entry_is_empty_string() {
        let ctx = make_ctx_with_cache(CACHE_KEY, "");
        let result = TaskStatusSegment.render(&Default::default(), &ctx);
        assert!(result.is_none());
    }

    #[test]
    fn uses_cached_value_when_present() {
        let ctx = make_ctx_with_cache(CACHE_KEY, "󱐌 3");
        let result = TaskStatusSegment.render(&Default::default(), &ctx).unwrap();
        assert_eq!(result.text, "󱐌 3");
        assert_eq!(result.cache_key.as_deref(), Some(CACHE_KEY));
    }

    #[test]
    fn read_last_log_entry_returns_none_for_missing_file() {
        let result = read_last_log_entry(&PathBuf::from("/nonexistent/path.jsonl"));
        assert!(result.is_none());
    }

    #[test]
    fn read_last_log_entry_returns_last_line() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("task.jsonl");
        std::fs::write(&path, "{\"started_at\":100,\"exit_code\":0,\"timed_out\":false}\n{\"started_at\":200,\"exit_code\":1,\"timed_out\":false}\n").unwrap();

        let entry = read_last_log_entry(&path).unwrap();
        assert_eq!(entry["started_at"], 200);
    }

    #[test]
    fn compute_summary_returns_none_when_no_tasks_toml() {
        // Override HOME to a temp dir with no tasks.toml.
        let tmp = TempDir::new().unwrap();
        std::env::set_var("HOME", tmp.path());
        let result = compute_task_summary();
        // Restore HOME — best effort in test.
        let _ = std::env::remove_var("HOME");
        assert!(result.is_none());
    }

    #[test]
    fn compute_summary_contains_task_count() {
        let tmp = TempDir::new().unwrap();
        let config_dir = tmp.path().join(".config/lynx");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("tasks.toml"),
            "[[task]]\nname=\"a\"\nrun=\"echo a\"\ncron=\"* * * * *\"\n\n[[task]]\nname=\"b\"\nrun=\"echo b\"\ncron=\"* * * * *\"\n",
        ).unwrap();

        std::env::set_var("HOME", tmp.path());
        let result = compute_task_summary();
        let _ = std::env::remove_var("HOME");

        let summary = result.unwrap();
        assert!(summary.contains("2"), "expected count 2, got: {summary}");
    }
}
