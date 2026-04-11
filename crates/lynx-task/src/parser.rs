use crate::schema::{Task, TasksFile, ValidatedTask};
use lynx_core::error::{LynxError, Result};
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

/// Parse and validate all tasks from a tasks.toml file path.
pub fn load_tasks(path: &Path) -> Result<Vec<ValidatedTask>> {
    let content = std::fs::read_to_string(path).map_err(LynxError::Io)?;
    parse_tasks_str(&content)
}

/// Parse and validate tasks from a TOML string.
pub fn parse_tasks_str(content: &str) -> Result<Vec<ValidatedTask>> {
    let file: TasksFile = toml::from_str(content)
        .map_err(|e| LynxError::Config(format!("tasks.toml parse error: {e}")))?;

    file.tasks
        .into_iter()
        .map(validate_task)
        .collect()
}

/// Validate a single task, returning a `ValidatedTask` or an error.
pub fn validate_task(task: Task) -> Result<ValidatedTask> {
    validate_required_fields(&task)?;
    validate_cron(&task.name, &task.cron)?;
    let timeout_duration = parse_timeout(&task.name, task.timeout.as_deref())?;

    Ok(ValidatedTask { task, timeout_duration })
}

// ── helpers ────────────────────────────────────────────────────────────────

fn validate_required_fields(task: &Task) -> Result<()> {
    if task.name.trim().is_empty() {
        return Err(LynxError::Config("task field 'name' is required and must not be empty".into()));
    }
    if task.run.trim().is_empty() {
        return Err(LynxError::Config(format!(
            "task '{}': field 'run' is required and must not be empty",
            task.name
        )));
    }
    if task.cron.trim().is_empty() {
        return Err(LynxError::Config(format!(
            "task '{}': field 'cron' is required and must not be empty",
            task.name
        )));
    }
    Ok(())
}

/// Validate a 5-field cron expression using the `cron` crate.
///
/// The `cron` crate expects 6-field expressions (sec min hr dom mon dow), so we
/// prepend "0" to make a "run at :00 seconds" schedule from a standard 5-field expr.
fn validate_cron(task_name: &str, expr: &str) -> Result<()> {
    let fields: Vec<&str> = expr.split_whitespace().collect();
    if fields.len() != 5 {
        return Err(LynxError::Config(format!(
            "task '{task_name}': cron expression must have exactly 5 fields \
             (min hr dom mon dow), got {}: '{expr}'",
            fields.len()
        )));
    }

    // Prepend seconds field "0" to form a 6-field expression for the cron crate.
    let six_field = format!("0 {expr}");
    cron::Schedule::from_str(&six_field).map_err(|e| {
        LynxError::Config(format!(
            "task '{task_name}': invalid cron expression '{expr}': {e}"
        ))
    })?;

    Ok(())
}

/// Parse optional human-duration strings like "60s", "5m", "1h".
fn parse_timeout(task_name: &str, timeout: Option<&str>) -> Result<Option<Duration>> {
    match timeout {
        None => Ok(None),
        Some(s) => {
            humantime::parse_duration(s)
                .map(Some)
                .map_err(|e| LynxError::Config(format!(
                    "task '{task_name}': invalid timeout '{s}': {e} \
                     (use formats like '60s', '5m', '1h')"
                )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_task(name: &str, run: &str, cron: &str) -> Task {
        crate::schema::Task {
            name: name.into(),
            description: String::new(),
            run: run.into(),
            cron: cron.into(),
            on_fail: crate::schema::OnFail::Log,
            timeout: None,
            log: true,
            enabled: true,
        }
    }

    // ── valid parses ─────────────────────────────────────────────────────

    #[test]
    fn valid_task_parses() {
        let toml = r#"
[[task]]
name = "hello"
run = "echo hello"
cron = "* * * * *"
"#;
        let tasks = parse_tasks_str(toml).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].task.name, "hello");
    }

    #[test]
    fn multiple_tasks_parse() {
        let toml = r#"
[[task]]
name = "a"
run = "echo a"
cron = "0 * * * *"

[[task]]
name = "b"
run = "echo b"
cron = "30 4 * * 1"
"#;
        let tasks = parse_tasks_str(toml).unwrap();
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn empty_file_returns_empty_vec() {
        let tasks = parse_tasks_str("").unwrap();
        assert!(tasks.is_empty());
    }

    // ── timeout parsing ──────────────────────────────────────────────────

    #[test]
    fn timeout_60s() {
        let mut t = make_task("t", "echo x", "* * * * *");
        t.timeout = Some("60s".into());
        let v = validate_task(t).unwrap();
        assert_eq!(v.timeout_duration, Some(Duration::from_secs(60)));
    }

    #[test]
    fn timeout_5m() {
        let mut t = make_task("t", "echo x", "* * * * *");
        t.timeout = Some("5m".into());
        let v = validate_task(t).unwrap();
        assert_eq!(v.timeout_duration, Some(Duration::from_secs(300)));
    }

    #[test]
    fn timeout_1h() {
        let mut t = make_task("t", "echo x", "* * * * *");
        t.timeout = Some("1h".into());
        let v = validate_task(t).unwrap();
        assert_eq!(v.timeout_duration, Some(Duration::from_secs(3600)));
    }

    #[test]
    fn no_timeout_is_none() {
        let t = make_task("t", "echo x", "* * * * *");
        let v = validate_task(t).unwrap();
        assert_eq!(v.timeout_duration, None);
    }

    #[test]
    fn invalid_timeout_errors() {
        let mut t = make_task("t", "echo x", "* * * * *");
        t.timeout = Some("banana".into());
        let err = validate_task(t).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("invalid timeout"), "got: {msg}");
    }

    // ── required field errors ────────────────────────────────────────────

    #[test]
    fn missing_name_errors() {
        let toml = r#"
[[task]]
name = ""
run = "echo x"
cron = "* * * * *"
"#;
        let err = parse_tasks_str(toml).unwrap_err();
        assert!(err.to_string().contains("'name'"), "got: {err}");
    }

    #[test]
    fn missing_run_errors() {
        // run defaults to "" in TOML if absent — we catch empty string
        let mut t = make_task("t", "", "* * * * *");
        t.run = String::new();
        let err = validate_task(t).unwrap_err();
        assert!(err.to_string().contains("'run'"), "got: {err}");
    }

    #[test]
    fn missing_cron_errors() {
        let t = make_task("t", "echo x", "");
        let err = validate_task(t).unwrap_err();
        assert!(err.to_string().contains("'cron'"), "got: {err}");
    }

    // ── cron validation ──────────────────────────────────────────────────

    #[test]
    fn invalid_cron_wrong_field_count() {
        let t = make_task("t", "echo x", "* * *");
        let err = validate_task(t).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("5 fields"), "got: {msg}");
    }

    #[test]
    fn invalid_cron_bad_value() {
        let t = make_task("t", "echo x", "99 * * * *");
        let err = validate_task(t).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("invalid cron"), "got: {msg}");
    }

    #[test]
    fn valid_cron_every_minute() {
        let t = make_task("t", "echo x", "* * * * *");
        assert!(validate_task(t).is_ok());
    }

    #[test]
    fn valid_cron_complex() {
        let t = make_task("t", "echo x", "30 4 1,15 * 5");
        assert!(validate_task(t).is_ok());
    }

    // ── defaults ─────────────────────────────────────────────────────────

    #[test]
    fn defaults_applied() {
        let toml = r#"
[[task]]
name = "minimal"
run = "echo min"
cron = "* * * * *"
"#;
        let tasks = parse_tasks_str(toml).unwrap();
        let t = &tasks[0].task;
        assert!(t.log);
        assert!(t.enabled);
        assert_eq!(t.on_fail, crate::schema::OnFail::Log);
        assert_eq!(t.description, "");
    }
}
