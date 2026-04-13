//! Workflow executor — sequential and concurrent step execution.
//!
//! Handles timeouts, on_fail policies, conditions, and signal handling.

use crate::job::{
    build_plan, epoch_ms, evaluate_condition, generate_job_id, persist_job_result,
    substitute_params,
};
use crate::runner;
use crate::schema::{OnFail, Step, Workflow};
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tracing::info;

/// Execution mode.
#[derive(Debug, Clone, PartialEq)]
pub enum ExecMode {
    Foreground,
    Background,
}

/// Result of a step execution.
#[derive(Debug, Clone)]
pub struct StepResult {
    pub name: String,
    pub status: StepStatus,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
}

/// Step execution status.
#[derive(Debug, Clone, PartialEq)]
pub enum StepStatus {
    Passed,
    Failed,
    Skipped,
    TimedOut,
}

/// Result of a full workflow execution.
#[derive(Debug, Clone)]
pub struct JobResult {
    pub workflow_name: String,
    pub job_id: String,
    pub success: bool,
    pub steps: Vec<StepResult>,
    pub started_at: u64,
    pub duration_ms: u64,
}

/// Execute a workflow with the given parameters.
pub async fn execute_workflow(
    workflow: &Workflow,
    params: &HashMap<String, String>,
    mode: ExecMode,
    log_dir: Option<PathBuf>,
) -> Result<JobResult> {
    let job_id = generate_job_id(&workflow.workflow.name);
    let started_at = epoch_ms();
    let mut step_results = Vec::new();
    let mut aborted = false;

    let plan = build_plan(&workflow.steps);

    for batch in &plan {
        if aborted {
            for step in batch {
                step_results.push(StepResult {
                    name: step.name.clone(),
                    status: StepStatus::Skipped,
                    exit_code: None,
                    duration_ms: 0,
                });
            }
            continue;
        }

        if batch.len() == 1 {
            let result = execute_step(&batch[0], params, &mode, log_dir.as_deref()).await;
            if result.status == StepStatus::Failed && batch[0].on_fail == OnFail::Abort {
                aborted = true;
            }
            step_results.push(result);
        } else {
            // Cap concurrent tasks to prevent unbounded spawning on large batches.
            let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(16));
            let handles: Vec<_> = batch
                .iter()
                .map(|step| {
                    let step = step.clone();
                    let params = params.clone();
                    let mode = mode.clone();
                    let ld = log_dir.clone();
                    let sem = semaphore.clone();
                    tokio::spawn(async move {
                        // Semaphore is Arc-owned by the enclosing scope — never closed.
                        let _permit = sem.acquire().await.expect("semaphore is never closed");
                        execute_step(&step, &params, &mode, ld.as_deref()).await
                    })
                })
                .collect();

            for handle in handles {
                let result = handle.await.unwrap_or_else(|_| StepResult {
                    name: "unknown".into(),
                    status: StepStatus::Failed,
                    exit_code: None,
                    duration_ms: 0,
                });
                if result.status == StepStatus::Failed {
                    if let Some(step) = batch.iter().find(|s| s.name == result.name) {
                        if step.on_fail == OnFail::Abort {
                            aborted = true;
                        }
                    }
                }
                step_results.push(result);
            }
        }
    }

    let duration_ms = epoch_ms() - started_at;
    let success = !aborted
        && step_results
            .iter()
            .all(|r| matches!(r.status, StepStatus::Passed | StepStatus::Skipped));

    let result = JobResult {
        workflow_name: workflow.workflow.name.clone(),
        job_id,
        success,
        steps: step_results,
        started_at,
        duration_ms,
    };

    if let Some(ref ld) = log_dir {
        persist_job_result(&result, ld);
    }

    Ok(result)
}

/// Execute a single step.
async fn execute_step(
    step: &Step,
    params: &HashMap<String, String>,
    mode: &ExecMode,
    _log_dir: Option<&std::path::Path>,
) -> StepResult {
    let start = epoch_ms();

    if let Some(ref condition) = step.condition {
        if !evaluate_condition(condition, params) {
            return StepResult {
                name: step.name.clone(),
                status: StepStatus::Skipped,
                exit_code: None,
                duration_ms: epoch_ms() - start,
            };
        }
    }

    let run_str = substitute_params(&step.run, params);

    let cmd = match runner::resolve(&step.runner, &run_str) {
        Ok(c) => c,
        Err(e) => {
            info!("step '{}': runner resolve failed: {}", step.name, e);
            return StepResult {
                name: step.name.clone(),
                status: StepStatus::Failed,
                exit_code: None,
                duration_ms: epoch_ms() - start,
            };
        }
    };

    let max_attempts = if step.on_fail == OnFail::Retry {
        step.retry_count.max(1)
    } else {
        1
    };

    for attempt in 0..max_attempts {
        if attempt > 0 {
            info!("step '{}': retry {}/{}", step.name, attempt + 1, max_attempts);
        }

        match run_command(&cmd, step, mode).await {
            Ok(code) => {
                if code == 0 {
                    return StepResult {
                        name: step.name.clone(),
                        status: StepStatus::Passed,
                        exit_code: Some(code),
                        duration_ms: epoch_ms() - start,
                    };
                }
                if attempt + 1 >= max_attempts {
                    return StepResult {
                        name: step.name.clone(),
                        status: StepStatus::Failed,
                        exit_code: Some(code),
                        duration_ms: epoch_ms() - start,
                    };
                }
            }
            Err(_) => {
                return StepResult {
                    name: step.name.clone(),
                    status: StepStatus::TimedOut,
                    exit_code: None,
                    duration_ms: epoch_ms() - start,
                };
            }
        }
    }

    StepResult {
        name: step.name.clone(),
        status: StepStatus::Failed,
        exit_code: None,
        duration_ms: epoch_ms() - start,
    }
}

/// Spawn and run a resolved command. Returns exit code or error on timeout.
async fn run_command(
    cmd: &runner::ResolvedCommand,
    step: &Step,
    _mode: &ExecMode,
) -> Result<i32, ()> {
    let mut command = tokio::process::Command::new(&cmd.binary);
    command.args(&cmd.args);

    for (k, v) in &step.env {
        command.env(k, v);
    }
    if let Some(ref cwd) = step.cwd {
        command.current_dir(cwd);
    }

    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(_) => return Err(()),
    };

    if let Some(timeout_sec) = step.timeout_sec {
        let timeout = std::time::Duration::from_secs(timeout_sec);
        match tokio::time::timeout(timeout, child.wait()).await {
            Ok(Ok(status)) => Ok(status.code().unwrap_or(-1)),
            Ok(Err(_)) => Err(()),
            Err(_) => {
                let _ = child.kill().await;
                Err(())
            }
        }
    } else {
        match child.wait().await {
            Ok(status) => Ok(status.code().unwrap_or(-1)),
            Err(_) => Err(()),
        }
    }
}

// ── Streaming executor (for TUI) ───────────────────────────────────────────

/// Events emitted during streaming execution.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    StepStarted { name: String },
    StepOutput { name: String, line: String, is_stderr: bool },
    StepFinished { name: String, status: StepStatus, duration_ms: u64 },
    Done { success: bool, duration_ms: u64 },
}

/// Execute a workflow, streaming events through a channel.
///
/// The caller should spawn this on a tokio task and consume events from the
/// receiver to drive a TUI or other live display.
pub async fn execute_workflow_streaming(
    workflow: &Workflow,
    params: &HashMap<String, String>,
    log_dir: Option<PathBuf>,
    tx: std::sync::mpsc::Sender<StreamEvent>,
) -> Result<JobResult> {
    let job_id = generate_job_id(&workflow.workflow.name);
    let started_at = epoch_ms();
    let mut step_results = Vec::new();
    let mut aborted = false;

    let plan = build_plan(&workflow.steps);

    for batch in &plan {
        if aborted {
            for step in batch {
                let _ = tx.send(StreamEvent::StepFinished {
                    name: step.name.clone(),
                    status: StepStatus::Skipped,
                    duration_ms: 0,
                });
                step_results.push(StepResult {
                    name: step.name.clone(),
                    status: StepStatus::Skipped,
                    exit_code: None,
                    duration_ms: 0,
                });
            }
            continue;
        }

        if batch.len() == 1 {
            let _ = tx.send(StreamEvent::StepStarted {
                name: batch[0].name.clone(),
            });
            let result = execute_step_streaming(&batch[0], params, &tx).await;
            let _ = tx.send(StreamEvent::StepFinished {
                name: result.name.clone(),
                status: result.status.clone(),
                duration_ms: result.duration_ms,
            });
            if result.status == StepStatus::Failed && batch[0].on_fail == OnFail::Abort {
                aborted = true;
            }
            step_results.push(result);
        } else {
            // Send StepStarted for all steps in the batch.
            for step in batch {
                let _ = tx.send(StreamEvent::StepStarted {
                    name: step.name.clone(),
                });
            }
            let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(16));
            let handles: Vec<_> = batch
                .iter()
                .map(|step| {
                    let step = step.clone();
                    let params = params.clone();
                    let tx = tx.clone();
                    let sem = semaphore.clone();
                    tokio::spawn(async move {
                        let _permit = sem.acquire().await.expect("semaphore is never closed");
                        let result = execute_step_streaming(&step, &params, &tx).await;
                        let _ = tx.send(StreamEvent::StepFinished {
                            name: result.name.clone(),
                            status: result.status.clone(),
                            duration_ms: result.duration_ms,
                        });
                        result
                    })
                })
                .collect();

            for handle in handles {
                let result = handle.await.unwrap_or_else(|_| StepResult {
                    name: "unknown".into(),
                    status: StepStatus::Failed,
                    exit_code: None,
                    duration_ms: 0,
                });
                if result.status == StepStatus::Failed {
                    if let Some(step) = batch.iter().find(|s| s.name == result.name) {
                        if step.on_fail == OnFail::Abort {
                            aborted = true;
                        }
                    }
                }
                step_results.push(result);
            }
        }
    }

    let duration_ms = epoch_ms() - started_at;
    let success = !aborted
        && step_results
            .iter()
            .all(|r| matches!(r.status, StepStatus::Passed | StepStatus::Skipped));

    let result = JobResult {
        workflow_name: workflow.workflow.name.clone(),
        job_id,
        success,
        steps: step_results,
        started_at,
        duration_ms,
    };

    if let Some(ref ld) = log_dir {
        persist_job_result(&result, ld);
    }

    let _ = tx.send(StreamEvent::Done { success, duration_ms });

    Ok(result)
}

/// Execute a single step with output streaming.
async fn execute_step_streaming(
    step: &Step,
    params: &HashMap<String, String>,
    tx: &std::sync::mpsc::Sender<StreamEvent>,
) -> StepResult {
    let start = epoch_ms();

    if let Some(ref condition) = step.condition {
        if !evaluate_condition(condition, params) {
            return StepResult {
                name: step.name.clone(),
                status: StepStatus::Skipped,
                exit_code: None,
                duration_ms: epoch_ms() - start,
            };
        }
    }

    let run_str = substitute_params(&step.run, params);

    let cmd = match runner::resolve(&step.runner, &run_str) {
        Ok(c) => c,
        Err(e) => {
            info!("step '{}': runner resolve failed: {}", step.name, e);
            let _ = tx.send(StreamEvent::StepOutput {
                name: step.name.clone(),
                line: format!("runner resolve failed: {e}"),
                is_stderr: true,
            });
            return StepResult {
                name: step.name.clone(),
                status: StepStatus::Failed,
                exit_code: None,
                duration_ms: epoch_ms() - start,
            };
        }
    };

    let max_attempts = if step.on_fail == OnFail::Retry {
        step.retry_count.max(1)
    } else {
        1
    };

    for attempt in 0..max_attempts {
        if attempt > 0 {
            let _ = tx.send(StreamEvent::StepOutput {
                name: step.name.clone(),
                line: format!("retry {}/{}", attempt + 1, max_attempts),
                is_stderr: false,
            });
        }

        match run_command_streaming(&cmd, step, &step.name, tx).await {
            Ok(code) => {
                if code == 0 {
                    return StepResult {
                        name: step.name.clone(),
                        status: StepStatus::Passed,
                        exit_code: Some(code),
                        duration_ms: epoch_ms() - start,
                    };
                }
                if attempt + 1 >= max_attempts {
                    return StepResult {
                        name: step.name.clone(),
                        status: StepStatus::Failed,
                        exit_code: Some(code),
                        duration_ms: epoch_ms() - start,
                    };
                }
            }
            Err(_) => {
                return StepResult {
                    name: step.name.clone(),
                    status: StepStatus::TimedOut,
                    exit_code: None,
                    duration_ms: epoch_ms() - start,
                };
            }
        }
    }

    StepResult {
        name: step.name.clone(),
        status: StepStatus::Failed,
        exit_code: None,
        duration_ms: epoch_ms() - start,
    }
}

/// Spawn a command with piped stdout/stderr, streaming lines through the channel.
async fn run_command_streaming(
    cmd: &runner::ResolvedCommand,
    step: &Step,
    step_name: &str,
    tx: &std::sync::mpsc::Sender<StreamEvent>,
) -> Result<i32, ()> {
    use tokio::io::{AsyncBufReadExt, BufReader};

    let mut command = tokio::process::Command::new(&cmd.binary);
    command.args(&cmd.args);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    for (k, v) in &step.env {
        command.env(k, v);
    }
    if let Some(ref cwd) = step.cwd {
        command.current_dir(cwd);
    }

    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(StreamEvent::StepOutput {
                name: step_name.to_string(),
                line: format!("spawn failed: {e}"),
                is_stderr: true,
            });
            return Err(());
        }
    };

    // Take stdout/stderr handles.
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let name_out = step_name.to_string();
    let tx_out = tx.clone();
    let stdout_handle = tokio::spawn(async move {
        if let Some(stdout) = stdout {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx_out.send(StreamEvent::StepOutput {
                    name: name_out.clone(),
                    line,
                    is_stderr: false,
                });
            }
        }
    });

    let name_err = step_name.to_string();
    let tx_err = tx.clone();
    let stderr_handle = tokio::spawn(async move {
        if let Some(stderr) = stderr {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx_err.send(StreamEvent::StepOutput {
                    name: name_err.clone(),
                    line,
                    is_stderr: true,
                });
            }
        }
    });

    let result = if let Some(timeout_sec) = step.timeout_sec {
        let timeout = std::time::Duration::from_secs(timeout_sec);
        match tokio::time::timeout(timeout, child.wait()).await {
            Ok(Ok(status)) => Ok(status.code().unwrap_or(-1)),
            Ok(Err(_)) => Err(()),
            Err(_) => {
                let _ = child.kill().await;
                let _ = tx.send(StreamEvent::StepOutput {
                    name: step_name.to_string(),
                    line: format!("timed out after {timeout_sec}s"),
                    is_stderr: true,
                });
                Err(())
            }
        }
    } else {
        match child.wait().await {
            Ok(status) => Ok(status.code().unwrap_or(-1)),
            Err(_) => Err(()),
        }
    };

    // Wait for output readers to finish.
    let _ = stdout_handle.await;
    let _ = stderr_handle.await;

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{RunnerType, WorkflowMeta};

    fn make_step(name: &str, run: &str) -> Step {
        Step {
            name: name.into(),
            runner: RunnerType::Sh,
            run: run.into(),
            confirm: false,
            timeout_sec: None,
            on_fail: OnFail::Abort,
            retry_count: 0,
            condition: None,
            depends_on: vec![],
            group: None,
            env: HashMap::new(),
            cwd: None,
        }
    }

    fn make_workflow(steps: Vec<Step>) -> Workflow {
        Workflow {
            workflow: WorkflowMeta {
                name: "test".into(),
                description: String::new(),
                context: None,
                params: vec![],
            },
            steps,
        }
    }

    #[tokio::test]
    async fn sequential_execution() {
        let wf = make_workflow(vec![make_step("s1", "true"), make_step("s2", "true")]);
        let result = execute_workflow(&wf, &HashMap::new(), ExecMode::Foreground, None)
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.steps.len(), 2);
    }

    #[tokio::test]
    async fn concurrent_group_execution() {
        let mut s1 = make_step("s1", "true");
        s1.group = Some("g1".into());
        let mut s2 = make_step("s2", "true");
        s2.group = Some("g1".into());
        let wf = make_workflow(vec![s1, s2, make_step("s3", "true")]);
        let result = execute_workflow(&wf, &HashMap::new(), ExecMode::Foreground, None)
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.steps.len(), 3);
    }

    #[tokio::test]
    async fn timeout_kills_step() {
        let mut step = make_step("slow", "sleep 60");
        step.timeout_sec = Some(1);
        let wf = make_workflow(vec![step]);
        let result = execute_workflow(&wf, &HashMap::new(), ExecMode::Foreground, None)
            .await
            .unwrap();
        assert!(!result.success);
        assert_eq!(result.steps[0].status, StepStatus::TimedOut);
    }

    #[tokio::test]
    async fn on_fail_abort_stops_workflow() {
        let wf = make_workflow(vec![make_step("fail", "false"), make_step("skip", "true")]);
        let result = execute_workflow(&wf, &HashMap::new(), ExecMode::Foreground, None)
            .await
            .unwrap();
        assert!(!result.success);
        assert_eq!(result.steps[0].status, StepStatus::Failed);
        assert_eq!(result.steps[1].status, StepStatus::Skipped);
    }

    #[tokio::test]
    async fn on_fail_continue_proceeds() {
        let mut s1 = make_step("fail", "false");
        s1.on_fail = OnFail::Continue;
        let wf = make_workflow(vec![s1, make_step("next", "true")]);
        let result = execute_workflow(&wf, &HashMap::new(), ExecMode::Foreground, None)
            .await
            .unwrap();
        assert!(!result.success);
        assert_eq!(result.steps[1].status, StepStatus::Passed);
    }

    #[tokio::test]
    async fn on_fail_retry() {
        let mut step = make_step("flaky", "false");
        step.on_fail = OnFail::Retry;
        step.retry_count = 2;
        let wf = make_workflow(vec![step]);
        let result = execute_workflow(&wf, &HashMap::new(), ExecMode::Foreground, None)
            .await
            .unwrap();
        assert!(!result.success);
    }

    #[tokio::test]
    async fn condition_skip() {
        let mut step = make_step("cond", "true");
        step.condition = Some("$deploy == yes".into());
        let wf = make_workflow(vec![step]);
        let result = execute_workflow(&wf, &HashMap::new(), ExecMode::Foreground, None)
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.steps[0].status, StepStatus::Skipped);
    }

    #[tokio::test]
    async fn streaming_emits_events_for_all_steps() {
        let mut s1 = make_step("lint", "echo lint-ok");
        s1.group = Some("checks".into());
        let mut s2 = make_step("test", "echo test-ok");
        s2.group = Some("checks".into());
        let s3 = make_step("build", "echo build-ok");

        let wf = make_workflow(vec![s1, s2, s3]);
        let (tx, rx) = std::sync::mpsc::channel();

        let result = execute_workflow_streaming(&wf, &HashMap::new(), None, tx)
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.steps.len(), 3);

        // Collect all events.
        let events: Vec<StreamEvent> = rx.try_iter().collect();

        // Every step must have a StepStarted event.
        let started: Vec<String> = events
            .iter()
            .filter_map(|e| match e {
                StreamEvent::StepStarted { name } => Some(name.clone()),
                _ => None,
            })
            .collect();
        assert!(started.contains(&"lint".to_string()), "lint missing StepStarted");
        assert!(started.contains(&"test".to_string()), "test missing StepStarted");
        assert!(started.contains(&"build".to_string()), "build missing StepStarted");

        // Every step must have a StepFinished event.
        let finished: Vec<String> = events
            .iter()
            .filter_map(|e| match e {
                StreamEvent::StepFinished { name, .. } => Some(name.clone()),
                _ => None,
            })
            .collect();
        assert!(finished.contains(&"lint".to_string()), "lint missing StepFinished");
        assert!(finished.contains(&"test".to_string()), "test missing StepFinished");
        assert!(finished.contains(&"build".to_string()), "build missing StepFinished");

        // Build must have output (echo build-ok).
        let build_output: Vec<&StreamEvent> = events
            .iter()
            .filter(|e| matches!(e, StreamEvent::StepOutput { name, .. } if name == "build"))
            .collect();
        assert!(!build_output.is_empty(), "build step should have output");

        // Must end with Done.
        assert!(
            matches!(events.last(), Some(StreamEvent::Done { success: true, .. })),
            "last event should be Done"
        );
    }

    #[tokio::test]
    async fn condition_passes() {
        let mut step = make_step("cond", "true");
        step.condition = Some("$deploy == yes".into());
        let wf = make_workflow(vec![step]);
        let mut params = HashMap::new();
        params.insert("deploy".into(), "yes".into());
        let result = execute_workflow(&wf, &params, ExecMode::Foreground, None)
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.steps[0].status, StepStatus::Passed);
    }
}
