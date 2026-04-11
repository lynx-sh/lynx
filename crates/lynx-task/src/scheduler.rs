use crate::schema::{OnFail, ValidatedTask};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::process::Command;
use tracing::{error, info, warn};

/// Maximum bytes of stdout/stderr captured per task run (~1000 lines at ~120 chars each).
const MAX_OUTPUT_BYTES: usize = 120_000;

/// One log entry written as a JSONL line to `~/.config/lynx/logs/tasks/<name>.jsonl`.
#[derive(Debug, Serialize)]
pub struct TaskRunLog {
    pub task: String,
    pub started_at: u64,
    pub duration_ms: u64,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub stdout_tail: String,
    pub stderr_tail: String,
}

/// Handle returned by `run_scheduler` — drop or abort to stop.
pub struct SchedulerHandle {
    pub abort: tokio::task::AbortHandle,
}

impl Drop for SchedulerHandle {
    fn drop(&mut self) {
        self.abort.abort();
    }
}

/// Start a background scheduler for the given tasks.
///
/// Each task fires on its cron schedule.  The handle must be kept alive;
/// dropping it shuts the scheduler down.
pub fn run_scheduler(tasks: Vec<ValidatedTask>, log_dir: PathBuf) -> SchedulerHandle {
    let handle = tokio::spawn(async move {
        scheduler_loop(tasks, &log_dir).await;
    });
    SchedulerHandle { abort: handle.abort_handle() }
}

async fn scheduler_loop(tasks: Vec<ValidatedTask>, log_dir: &Path) {
    // Build one sleep-loop per task, all running concurrently.
    let futures: Vec<_> = tasks
        .into_iter()
        .map(|vt| {
            let log_dir = log_dir.to_path_buf();
            tokio::spawn(async move {
                task_loop(vt, log_dir).await;
            })
        })
        .collect();

    futures::future::join_all(futures).await;
}

async fn task_loop(vt: ValidatedTask, log_dir: PathBuf) {
    let schedule = match cron::Schedule::from_str(&format!("0 {}", vt.task.cron)) {
        Ok(s) => s,
        Err(e) => {
            error!(task = %vt.task.name, "invalid cron expression: {e}");
            return;
        }
    };

    loop {
        // Find next fire time.
        let now = chrono::Utc::now();
        let Some(next) = schedule.upcoming(chrono::Utc).next() else {
            warn!(task = %vt.task.name, "no upcoming schedule — stopping task loop");
            return;
        };

        let wait = (next - now).to_std().unwrap_or(Duration::ZERO);
        tokio::time::sleep(wait).await;

        if !vt.task.enabled {
            continue;
        }

        info!(task = %vt.task.name, "firing");
        let log = run_task(&vt).await;

        let failed = log.exit_code.map(|c| c != 0).unwrap_or(true) || log.timed_out;
        if failed {
            match vt.task.on_fail {
                OnFail::Notify => send_notification(&vt.task.name, &log).await,
                OnFail::Log => {
                    warn!(task = %vt.task.name, exit_code = ?log.exit_code, timed_out = log.timed_out, "task failed");
                }
                OnFail::Ignore => {}
            }
        }

        if vt.task.log {
            write_log(&log_dir, &log).await;
        }
    }
}

/// Run the task command, enforce timeout, return a log entry.
async fn run_task(vt: &ValidatedTask) -> TaskRunLog {
    let started_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let start = tokio::time::Instant::now();

    let mut child = match Command::new("sh")
        .arg("-c")
        .arg(&vt.task.run)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            error!(task = %vt.task.name, "failed to spawn: {e}");
            return TaskRunLog {
                task: vt.task.name.clone(),
                started_at,
                duration_ms: 0,
                exit_code: Some(-1),
                timed_out: false,
                stdout_tail: String::new(),
                stderr_tail: format!("spawn error: {e}"),
            };
        }
    };

    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();

    // Collect stdout/stderr with a size cap.
    let stdout_fut = read_capped(stdout_handle, MAX_OUTPUT_BYTES);
    let stderr_fut = read_capped(stderr_handle, MAX_OUTPUT_BYTES);

    let (wait_result, stdout_bytes, stderr_bytes) = if let Some(timeout) = vt.timeout_duration {
        let result = tokio::time::timeout(timeout, async {
            let (a, b) = tokio::join!(stdout_fut, stderr_fut);
            let status = child.wait().await;
            (status, a, b)
        })
        .await;

        match result {
            Ok((status, a, b)) => (status, a, b),
            Err(_elapsed) => {
                // Timed out — kill the process.
                let _ = child.kill().await;
                let _ = child.wait().await;
                let duration_ms = start.elapsed().as_millis() as u64;
                return TaskRunLog {
                    task: vt.task.name.clone(),
                    started_at,
                    duration_ms,
                    exit_code: None,
                    timed_out: true,
                    stdout_tail: String::new(),
                    stderr_tail: String::new(),
                };
            }
        }
    } else {
        let (a, b) = tokio::join!(stdout_fut, stderr_fut);
        let status = child.wait().await;
        (status, a, b)
    };

    let duration_ms = start.elapsed().as_millis() as u64;
    let exit_code = wait_result.ok().and_then(|s| s.code());

    TaskRunLog {
        task: vt.task.name.clone(),
        started_at,
        duration_ms,
        exit_code,
        timed_out: false,
        stdout_tail: tail_lines(&stdout_bytes, 1000),
        stderr_tail: tail_lines(&stderr_bytes, 1000),
    }
}

async fn read_capped(
    handle: Option<impl tokio::io::AsyncRead + Unpin>,
    cap: usize,
) -> Vec<u8> {
    let Some(reader) = handle else {
        return Vec::new();
    };
    let mut buf = Vec::new();
    // read_to_end via take() ensures we collect all output up to the cap.
    use tokio::io::AsyncReadExt;
    let _ = reader.take(cap as u64).read_to_end(&mut buf).await;
    buf
}

fn tail_lines(bytes: &[u8], max_lines: usize) -> String {
    let s = String::from_utf8_lossy(bytes);
    let lines: Vec<&str> = s.lines().collect();
    if lines.len() <= max_lines {
        return s.into_owned();
    }
    lines[lines.len() - max_lines..].join("\n")
}

async fn write_log(log_dir: &Path, log: &TaskRunLog) {
    let dir = log_dir.join("tasks");
    if let Err(e) = tokio::fs::create_dir_all(&dir).await {
        error!("failed to create log dir: {e}");
        return;
    }

    let path = dir.join(format!("{}.jsonl", log.task));
    let line = match serde_json::to_string(log) {
        Ok(s) => s + "\n",
        Err(e) => {
            error!("failed to serialize task log: {e}");
            return;
        }
    };

    use tokio::io::AsyncWriteExt;
    match tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await
    {
        Ok(mut f) => {
            if let Err(e) = f.write_all(line.as_bytes()).await {
                error!("failed to write task log: {e}");
                return;
            }
            if let Err(e) = f.flush().await {
                error!("failed to flush task log: {e}");
            }
        }
        Err(e) => error!("failed to open task log {}: {e}", path.display()),
    }
}

async fn send_notification(task_name: &str, log: &TaskRunLog) {
    let message = format!(
        "Lynx task '{}' failed (exit {:?}, {}ms)",
        task_name,
        log.exit_code,
        log.duration_ms
    );

    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("osascript")
            .arg("-e")
            .arg(format!(
                r#"display notification "{message}" with title "Lynx Task Failed""#
            ))
            .status()
            .await;
    }

    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("notify-send")
            .arg("Lynx Task Failed")
            .arg(&message)
            .status()
            .await;
    }
}

// ── futures dep for join_all ───────────────────────────────────────────────
// We need futures::future::join_all.  Use tokio's select/join where possible,
// but for the Vec case we need the futures crate.
mod futures {
    pub mod future {
        pub async fn join_all<F>(futs: Vec<F>)
        where
            F: std::future::Future + Send + 'static,
            F::Output: Send + 'static,
        {
            let handles: Vec<_> = futs
                .into_iter()
                .map(|f| tokio::spawn(f))
                .collect();
            for h in handles {
                let _ = h.await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{OnFail, Task};

    fn make_validated(run: &str, timeout: Option<Duration>) -> ValidatedTask {
        ValidatedTask {
            task: Task {
                name: "test_task".into(),
                description: String::new(),
                run: run.into(),
                cron: "* * * * *".into(),
                on_fail: OnFail::Log,
                timeout: timeout.map(|d| humantime::format_duration(d).to_string()),
                log: true,
                enabled: true,
            },
            timeout_duration: timeout,
        }
    }

    #[tokio::test]
    async fn task_runs_and_logs_exit_code() {
        let vt = make_validated("exit 0", None);
        let log = run_task(&vt).await;
        assert_eq!(log.exit_code, Some(0));
        assert!(!log.timed_out);
    }

    #[tokio::test]
    async fn task_captures_stdout() {
        let vt = make_validated("echo hello_world", None);
        let log = run_task(&vt).await;
        assert!(log.stdout_tail.contains("hello_world"), "got: {:?}", log.stdout_tail);
    }

    #[tokio::test]
    async fn failing_task_captures_exit_code() {
        let vt = make_validated("exit 42", None);
        let log = run_task(&vt).await;
        assert_eq!(log.exit_code, Some(42));
    }

    #[tokio::test]
    async fn task_exceeding_timeout_is_killed() {
        let vt = make_validated("sleep 60", Some(Duration::from_millis(100)));
        let log = run_task(&vt).await;
        assert!(log.timed_out, "expected timed_out=true");
        assert!(log.exit_code.is_none());
        assert!(log.duration_ms < 5_000, "should complete quickly after kill");
    }

    #[tokio::test]
    async fn log_written_to_jsonl() {
        let tmp = tempfile::tempdir().unwrap();
        let vt = make_validated("echo written", None);
        let log = run_task(&vt).await;
        write_log(tmp.path(), &log).await;

        let log_path = tmp.path().join("tasks/test_task.jsonl");
        assert!(log_path.exists(), "log file not created");
        let content = std::fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("written") || content.contains("test_task"));
    }

    #[test]
    fn tail_lines_caps_at_limit() {
        let many = (0..2000).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        let result = tail_lines(many.as_bytes(), 1000);
        assert_eq!(result.lines().count(), 1000);
        assert!(result.contains("line 1999"));
    }

    #[test]
    fn tail_lines_short_output_unchanged() {
        let short = "a\nb\nc";
        let result = tail_lines(short.as_bytes(), 1000);
        assert_eq!(result, short);
    }
}
