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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecMode {
    Foreground,
    Background,
}

/// Maximum lines buffered per step (stdout or stderr). Lines beyond this cap are
/// dropped (newest-first drop) and a warning is emitted.
pub const STEP_OUTPUT_LINE_CAP: usize = 10_000;

/// Number of stderr tail lines included in the agent failure excerpt.
pub const AGENT_FAILURE_EXCERPT_LINES: usize = 20;

/// Result of a step execution.
#[derive(Debug, Clone)]
pub struct StepResult {
    pub name: String,
    pub status: StepStatus,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    /// Buffered stdout lines (capped at [`STEP_OUTPUT_LINE_CAP`]).
    pub output_lines: Vec<String>,
    /// Buffered stderr lines (capped at [`STEP_OUTPUT_LINE_CAP`]).
    pub stderr_lines: Vec<String>,
}

/// Step execution status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepStatus {
    Passed,
    Failed,
    Skipped,
    TimedOut,
}

impl StepStatus {
    /// Unicode icon representing this status.
    pub fn icon(&self) -> &'static str {
        match self {
            StepStatus::Passed => "\u{2713}",   // ✓
            StepStatus::Failed => "\u{2717}",   // ✗
            StepStatus::Skipped => "\u{2014}",  // —
            StepStatus::TimedOut => "\u{23f0}", // ⏰
        }
    }
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

/// Events emitted during streaming execution.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    StepStarted {
        name: String,
    },
    StepOutput {
        name: String,
        line: String,
        is_stderr: bool,
    },
    StepFinished {
        name: String,
        status: StepStatus,
        duration_ms: u64,
    },
    Done {
        success: bool,
        duration_ms: u64,
    },
}

/// Execute a workflow with the given parameters.
pub async fn execute_workflow(
    workflow: &Workflow,
    params: &HashMap<String, String>,
    mode: ExecMode,
    log_dir: Option<PathBuf>,
) -> Result<JobResult> {
    execute_workflow_impl(workflow, params, mode, log_dir, None).await
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
    execute_workflow_impl(workflow, params, ExecMode::Foreground, log_dir, Some(tx)).await
}

async fn execute_workflow_impl(
    workflow: &Workflow,
    params: &HashMap<String, String>,
    mode: ExecMode,
    log_dir: Option<PathBuf>,
    stream_tx: Option<std::sync::mpsc::Sender<StreamEvent>>,
) -> Result<JobResult> {
    let job_id = generate_job_id(&workflow.workflow.name);
    let started_at = epoch_ms();
    let mut step_results = Vec::new();
    let mut aborted = false;
    let agent_mode = crate::context::is_agent_context();

    // Resolve workflow-level path once (supports param substitution).
    let workflow_path: Option<String> = workflow
        .workflow
        .path
        .as_deref()
        .map(|p| substitute_params(p, params));

    let plan = build_plan(&workflow.steps);

    for batch in &plan {
        if aborted {
            for step in batch {
                emit(
                    &stream_tx,
                    StreamEvent::StepFinished {
                        name: step.name.clone(),
                        status: StepStatus::Skipped,
                        duration_ms: 0,
                    },
                );
                step_results.push(StepResult {
                    name: step.name.clone(),
                    status: StepStatus::Skipped,
                    exit_code: None,
                    duration_ms: 0,
                    output_lines: vec![],
                    stderr_lines: vec![],
                });
            }
            continue;
        }

        if batch.len() == 1 {
            let step = &batch[0];
            emit(
                &stream_tx,
                StreamEvent::StepStarted {
                    name: step.name.clone(),
                },
            );
            let result = execute_step(step, params, &mode, stream_tx.clone(), workflow_path.as_deref(), agent_mode).await;
            emit(
                &stream_tx,
                StreamEvent::StepFinished {
                    name: result.name.clone(),
                    status: result.status.clone(),
                    duration_ms: result.duration_ms,
                },
            );
            if should_abort_on_failed_result(batch, &result) {
                aborted = true;
            }
            step_results.push(result);
            continue;
        }

        for step in batch {
            emit(
                &stream_tx,
                StreamEvent::StepStarted {
                    name: step.name.clone(),
                },
            );
        }

        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(16));
        let handles: Vec<_> = batch
            .iter()
            .map(|step| {
                let step = step.clone();
                let params = params.clone();
                let mode = mode.clone();
                let sem = semaphore.clone();
                let tx = stream_tx.clone();
                let wf_path = workflow_path.clone();
                tokio::spawn(async move {
                    // Semaphore is Arc-owned by the enclosing scope — never closed.
                    let _permit = sem.acquire().await.expect("semaphore is never closed");
                    let result = execute_step(&step, &params, &mode, tx.clone(), wf_path.as_deref(), agent_mode).await;
                    if let Some(sender) = tx {
                        let _ = sender.send(StreamEvent::StepFinished {
                            name: result.name.clone(),
                            status: result.status.clone(),
                            duration_ms: result.duration_ms,
                        });
                    }
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
                output_lines: vec![],
                stderr_lines: vec![],
            });
            if should_abort_on_failed_result(batch, &result) {
                aborted = true;
            }
            step_results.push(result);
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

    emit(
        &stream_tx,
        StreamEvent::Done {
            success,
            duration_ms,
        },
    );

    Ok(result)
}

fn should_abort_on_failed_result(batch: &[Step], result: &StepResult) -> bool {
    if result.status != StepStatus::Failed {
        return false;
    }
    batch
        .iter()
        .find(|step| step.name == result.name)
        .is_some_and(|step| step.on_fail == OnFail::Abort)
}

fn emit(tx: &Option<std::sync::mpsc::Sender<StreamEvent>>, event: StreamEvent) {
    if let Some(sender) = tx {
        let _ = sender.send(event);
    }
}

async fn collect_stream<R>(
    pipe: Option<R>,
    step_name: String,
    is_stderr: bool,
    tx: Option<std::sync::mpsc::Sender<StreamEvent>>,
) -> Vec<String>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    use tokio::io::{AsyncBufReadExt, BufReader};
    let mut collected: Vec<String> = Vec::new();
    if let Some(pipe) = pipe {
        let reader = BufReader::new(pipe);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(ref tx) = tx {
                let _ = tx.send(StreamEvent::StepOutput {
                    name: step_name.clone(),
                    line: line.clone(),
                    is_stderr,
                });
            }
            if collected.len() < STEP_OUTPUT_LINE_CAP {
                collected.push(line);
            } else if collected.len() == STEP_OUTPUT_LINE_CAP {
                let stream = if is_stderr { "stderr" } else { "stdout" };
                tracing::warn!(step = %step_name, "{stream} exceeded {STEP_OUTPUT_LINE_CAP} lines; dropping further lines from log buffer");
            }
        }
    }
    collected
}

/// In agent mode, emit a single `StepOutput` event with the last
/// [`AGENT_FAILURE_EXCERPT_LINES`] lines of stderr so the agent can diagnose
/// the failure without receiving every output line.
fn emit_agent_failure_excerpt(
    agent_mode: bool,
    step_name: &str,
    stderr_lines: &[String],
    tx: &Option<std::sync::mpsc::Sender<StreamEvent>>,
) {
    if !agent_mode {
        return;
    }
    let tail: Vec<&str> = stderr_lines
        .iter()
        .rev()
        .take(AGENT_FAILURE_EXCERPT_LINES)
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let excerpt = if tail.is_empty() {
        format!("--- failure excerpt (last {AGENT_FAILURE_EXCERPT_LINES} lines) ---\n(no stderr output)")
    } else {
        format!(
            "--- failure excerpt (last {AGENT_FAILURE_EXCERPT_LINES} lines) ---\n{}",
            tail.join("\n")
        )
    };
    emit(
        tx,
        StreamEvent::StepOutput {
            name: step_name.to_string(),
            line: excerpt,
            is_stderr: true,
        },
    );
}

/// Execute a single step.
/// `workflow_path` is the resolved `[workflow] path` value — used as the cwd
/// fallback when the step does not specify its own `cwd`.
async fn execute_step(
    step: &Step,
    params: &HashMap<String, String>,
    mode: &ExecMode,
    stream_tx: Option<std::sync::mpsc::Sender<StreamEvent>>,
    workflow_path: Option<&str>,
    agent_mode: bool,
) -> StepResult {
    let start = epoch_ms();

    if let Some(ref condition) = step.condition {
        if !evaluate_condition(condition, params) {
            return StepResult {
                name: step.name.clone(),
                status: StepStatus::Skipped,
                exit_code: None,
                duration_ms: epoch_ms() - start,
                output_lines: vec![],
                stderr_lines: vec![],
            };
        }
    }

    let run_str = substitute_params(&step.run, params);

    let cmd = match runner::resolve(&step.runner, &run_str) {
        Ok(c) => c,
        Err(e) => {
            info!("step '{}': runner resolve failed: {}", step.name, e);
            emit(
                &stream_tx,
                StreamEvent::StepOutput {
                    name: step.name.clone(),
                    line: format!("runner resolve failed: {e}"),
                    is_stderr: true,
                },
            );
            return StepResult {
                name: step.name.clone(),
                status: StepStatus::Failed,
                exit_code: None,
                duration_ms: epoch_ms() - start,
                output_lines: vec![],
                stderr_lines: vec![format!("runner resolve failed: {e}")],
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
            info!(
                "step '{}': retry {}/{}",
                step.name,
                attempt + 1,
                max_attempts
            );
            emit(
                &stream_tx,
                StreamEvent::StepOutput {
                    name: step.name.clone(),
                    line: format!("retry {}/{}", attempt + 1, max_attempts),
                    is_stderr: false,
                },
            );
        }

        // Resolve effective cwd: step-level cwd (param-substituted) wins,
        // then workflow-level path (param-substituted), then none.
        let step_cwd = step.cwd.as_deref().map(|c| substitute_params(c, params));
        let resolved_cwd: Option<String> = step_cwd
            .or_else(|| workflow_path.map(|p| substitute_params(p, params)));

        match run_command(&cmd, step, mode, &step.name, stream_tx.as_ref(), resolved_cwd.as_deref(), agent_mode).await {
            Ok((code, out, err)) => {
                if code == 0 {
                    return StepResult {
                        name: step.name.clone(),
                        status: StepStatus::Passed,
                        exit_code: Some(code),
                        duration_ms: epoch_ms() - start,
                        output_lines: out,
                        stderr_lines: err,
                    };
                }
                if attempt + 1 >= max_attempts {
                    emit_agent_failure_excerpt(agent_mode, &step.name, &err, &stream_tx);
                    return StepResult {
                        name: step.name.clone(),
                        status: StepStatus::Failed,
                        exit_code: Some(code),
                        duration_ms: epoch_ms() - start,
                        output_lines: out,
                        stderr_lines: err,
                    };
                }
            }
            Err(_) => {
                emit_agent_failure_excerpt(agent_mode, &step.name, &[], &stream_tx);
                return StepResult {
                    name: step.name.clone(),
                    status: StepStatus::TimedOut,
                    exit_code: None,
                    duration_ms: epoch_ms() - start,
                    output_lines: vec![],
                    stderr_lines: vec![],
                };
            }
        }
    }

    StepResult {
        name: step.name.clone(),
        status: StepStatus::Failed,
        exit_code: None,
        duration_ms: epoch_ms() - start,
        output_lines: vec![],
        stderr_lines: vec![],
    }
}

/// Spawn and run a resolved command.
///
/// Returns `(exit_code, stdout_lines, stderr_lines)` on success, or `Err(())`
/// on spawn failure or timeout. Lines are buffered up to [`STEP_OUTPUT_LINE_CAP`]
/// per stream; excess lines are dropped (newest-first) with a tracing warning.
async fn run_command(
    cmd: &runner::ResolvedCommand,
    step: &Step,
    _mode: &ExecMode,
    step_name: &str,
    stream_tx: Option<&std::sync::mpsc::Sender<StreamEvent>>,
    effective_cwd: Option<&str>,
    agent_mode: bool,
) -> Result<(i32, Vec<String>, Vec<String>), ()> {
    // Interactive foreground: try PTY first so the child sees a real terminal.
    // Agent mode: skip PTY — output is suppressed anyway, piped is cheaper.
    if !agent_mode {
        if let Some(tx) = stream_tx {
            match crate::pty_runner::run_in_pty(cmd, step, effective_cwd, tx, step_name).await {
                Ok(result) => return Ok(result),
                Err(()) => {
                    // PTY failed (e.g. container without /dev/ptmx) — fall through to piped.
                    tracing::warn!(step = %step_name, "PTY unavailable; falling back to Stdio::piped()");
                }
            }
        }
    }

    let mut command = tokio::process::Command::new(&cmd.binary);
    command.args(&cmd.args);

    // Always pipe so we can buffer output for the job log.
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    for (k, v) in &step.env {
        command.env(k, v);
    }
    if let Some(cwd) = effective_cwd {
        command.current_dir(cwd);
    }

    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(e) => {
            if let Some(tx) = stream_tx {
                let _ = tx.send(StreamEvent::StepOutput {
                    name: step_name.to_string(),
                    line: format!("spawn failed: {e}"),
                    is_stderr: true,
                });
            }
            return Err(());
        }
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // In agent mode we suppress StepOutput events — collect only, no send.
    let tx_out = if agent_mode { None } else { stream_tx.cloned() };
    let tx_err = if agent_mode { None } else { stream_tx.cloned() };
    let stdout_handle: tokio::task::JoinHandle<Vec<String>> =
        tokio::spawn(collect_stream(stdout, step_name.to_string(), false, tx_out));
    let stderr_handle: tokio::task::JoinHandle<Vec<String>> =
        tokio::spawn(collect_stream(stderr, step_name.to_string(), true, tx_err));

    let exit_result = if let Some(timeout_sec) = step.timeout_sec {
        let timeout = std::time::Duration::from_secs(timeout_sec);
        match tokio::time::timeout(timeout, child.wait()).await {
            Ok(Ok(status)) => Ok(status.code().unwrap_or(-1)),
            Ok(Err(_)) => Err(()),
            Err(_) => {
                let _ = child.kill().await;
                if let Some(tx) = stream_tx {
                    let _ = tx.send(StreamEvent::StepOutput {
                        name: step_name.to_string(),
                        line: format!("timed out after {timeout_sec}s"),
                        is_stderr: true,
                    });
                }
                Err(())
            }
        }
    } else {
        match child.wait().await {
            Ok(status) => Ok(status.code().unwrap_or(-1)),
            Err(_) => Err(()),
        }
    };

    let out = stdout_handle.await.unwrap_or_default();
    let err = stderr_handle.await.unwrap_or_default();

    exit_result.map(|code| (code, out, err))
}
