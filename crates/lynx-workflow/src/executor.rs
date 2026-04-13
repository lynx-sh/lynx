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

/// Result of a step execution.
#[derive(Debug, Clone)]
pub struct StepResult {
    pub name: String,
    pub status: StepStatus,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
}

/// Step execution status.
#[derive(Debug, Clone, PartialEq, Eq)]
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
            let result = execute_step(step, params, &mode, stream_tx.clone()).await;
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
                tokio::spawn(async move {
                    // Semaphore is Arc-owned by the enclosing scope — never closed.
                    let _permit = sem.acquire().await.expect("semaphore is never closed");
                    let result = execute_step(&step, &params, &mode, tx.clone()).await;
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

/// Execute a single step.
async fn execute_step(
    step: &Step,
    params: &HashMap<String, String>,
    mode: &ExecMode,
    stream_tx: Option<std::sync::mpsc::Sender<StreamEvent>>,
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

        match run_command(&cmd, step, mode, &step.name, stream_tx.as_ref()).await {
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
    step_name: &str,
    stream_tx: Option<&std::sync::mpsc::Sender<StreamEvent>>,
) -> Result<i32, ()> {
    let mut command = tokio::process::Command::new(&cmd.binary);
    command.args(&cmd.args);

    if stream_tx.is_some() {
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
    }

    for (k, v) in &step.env {
        command.env(k, v);
    }
    if let Some(ref cwd) = step.cwd {
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

    let mut stdout_handle = None;
    let mut stderr_handle = None;

    if let Some(tx) = stream_tx {
        use tokio::io::{AsyncBufReadExt, BufReader};

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let name_out = step_name.to_string();
        let tx_out = tx.clone();
        stdout_handle = Some(tokio::spawn(async move {
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
        }));

        let name_err = step_name.to_string();
        let tx_err = tx.clone();
        stderr_handle = Some(tokio::spawn(async move {
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
        }));
    }

    let result = if let Some(timeout_sec) = step.timeout_sec {
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

    if let Some(handle) = stdout_handle {
        let _ = handle.await;
    }
    if let Some(handle) = stderr_handle {
        let _ = handle.await;
    }

    result
}
