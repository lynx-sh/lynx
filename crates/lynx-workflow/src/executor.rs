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
                        let _permit = sem.acquire().await.expect("semaphore closed");
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
