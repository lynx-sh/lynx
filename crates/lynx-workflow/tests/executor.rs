use std::collections::HashMap;

/// Temporarily sets or removes an env var and restores it on drop.
struct EnvGuard {
    key: &'static str,
    saved: Option<std::ffi::OsString>,
}
impl EnvGuard {
    fn unset(key: &'static str) -> Self {
        let saved = std::env::var_os(key);
        std::env::remove_var(key);
        EnvGuard { key, saved }
    }
    fn set(key: &'static str, value: &str) -> Self {
        let saved = std::env::var_os(key);
        std::env::set_var(key, value);
        EnvGuard { key, saved }
    }
}
impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.saved {
            Some(val) => std::env::set_var(self.key, val),
            None => std::env::remove_var(self.key),
        }
    }
}

use lynx_workflow::executor::{
    execute_workflow, execute_workflow_streaming, ExecMode, StepStatus, StreamEvent,
};
use lynx_workflow::schema::{OnFail, RunnerType, Step, Workflow, WorkflowMeta};

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
            path: None,
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
    // Force interactive context so StepOutput events are not suppressed in agent env.
    let _g1 = EnvGuard::unset("CLAUDECODE");
    let _g2 = EnvGuard::unset("CURSOR_CLI");

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

    let events: Vec<StreamEvent> = rx.try_iter().collect();

    let started: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::StepStarted { name } => Some(name.clone()),
            _ => None,
        })
        .collect();
    assert!(
        started.contains(&"lint".to_string()),
        "lint missing StepStarted"
    );
    assert!(
        started.contains(&"test".to_string()),
        "test missing StepStarted"
    );
    assert!(
        started.contains(&"build".to_string()),
        "build missing StepStarted"
    );

    let finished: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::StepFinished { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect();
    assert!(
        finished.contains(&"lint".to_string()),
        "lint missing StepFinished"
    );
    assert!(
        finished.contains(&"test".to_string()),
        "test missing StepFinished"
    );
    assert!(
        finished.contains(&"build".to_string()),
        "build missing StepFinished"
    );

    let build_output: Vec<&StreamEvent> = events
        .iter()
        .filter(|e| matches!(e, StreamEvent::StepOutput { name, .. } if name == "build"))
        .collect();
    assert!(!build_output.is_empty(), "build step should have output");

    assert!(
        matches!(events.last(), Some(StreamEvent::Done { success: true, .. })),
        "last event should be Done"
    );
}

#[tokio::test]
async fn streaming_captures_stderr_output() {
    // Force interactive context so stderr StepOutput events are not suppressed.
    let _g1 = EnvGuard::unset("CLAUDECODE");
    let _g2 = EnvGuard::unset("CURSOR_CLI");

    let wf = make_workflow(vec![make_step("build", "echo 'Finished release' >&2")]);
    let (tx, rx) = std::sync::mpsc::channel();
    let result = execute_workflow_streaming(&wf, &HashMap::new(), None, tx)
        .await
        .unwrap();
    assert!(result.success);

    let events: Vec<StreamEvent> = rx.try_iter().collect();
    // PTY merges stdout and stderr into one stream (is_stderr is always false).
    // Assert the output line was captured regardless of the is_stderr flag.
    let captured = events.iter().any(|e| {
        matches!(e, StreamEvent::StepOutput { line, .. } if line.contains("Finished release"))
    });
    assert!(
        captured,
        "stderr output should be captured (PTY merges stdout+stderr); events: {events:?}"
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

// ─── B26 agent-verbosity tests ───────────────────────────────────────────────

#[tokio::test]
async fn agent_mode_suppresses_step_output() {
    let _g1 = EnvGuard::set("CLAUDECODE", "1");
    let _g2 = EnvGuard::unset("CURSOR_CLI");

    // Step prints 5 lines to stdout.
    let wf = make_workflow(vec![make_step(
        "printer",
        "printf 'line1\nline2\nline3\nline4\nline5\n'",
    )]);
    let (tx, rx) = std::sync::mpsc::channel();
    let result = execute_workflow_streaming(&wf, &HashMap::new(), None, tx)
        .await
        .unwrap();
    assert!(result.success);

    let events: Vec<StreamEvent> = rx.try_iter().collect();
    // StepStarted and StepFinished must arrive; StepOutput must NOT.
    assert!(events.iter().any(|e| matches!(e, StreamEvent::StepStarted { .. })));
    assert!(events.iter().any(|e| matches!(e, StreamEvent::StepFinished { .. })));
    let output_events: Vec<&StreamEvent> = events
        .iter()
        .filter(|e| matches!(e, StreamEvent::StepOutput { .. }))
        .collect();
    assert!(
        output_events.is_empty(),
        "agent mode must suppress StepOutput; got: {output_events:?}"
    );
}

#[tokio::test]
async fn agent_mode_failure_emits_excerpt() {
    let _g1 = EnvGuard::set("CLAUDECODE", "1");
    let _g2 = EnvGuard::unset("CURSOR_CLI");

    // Step writes to stderr and exits non-zero.
    let wf = make_workflow(vec![make_step(
        "fail",
        "echo 'error: build failed' >&2; exit 1",
    )]);
    let (tx, rx) = std::sync::mpsc::channel();
    let _result = execute_workflow_streaming(&wf, &HashMap::new(), None, tx)
        .await
        .unwrap();

    let events: Vec<StreamEvent> = rx.try_iter().collect();
    let output_events: Vec<&StreamEvent> = events
        .iter()
        .filter(|e| matches!(e, StreamEvent::StepOutput { .. }))
        .collect();
    assert_eq!(
        output_events.len(),
        1,
        "agent failure must emit exactly one excerpt StepOutput; got: {output_events:?}"
    );
    if let StreamEvent::StepOutput { line, .. } = output_events[0] {
        assert!(
            line.contains("failure excerpt"),
            "excerpt must contain 'failure excerpt'; got: {line:?}"
        );
    }
}

#[tokio::test]
async fn interactive_mode_streams_all_output() {
    let _g1 = EnvGuard::unset("CLAUDECODE");
    let _g2 = EnvGuard::unset("CURSOR_CLI");

    let wf = make_workflow(vec![make_step(
        "printer",
        "printf 'a\nb\nc\n'",
    )]);
    let (tx, rx) = std::sync::mpsc::channel();
    let result = execute_workflow_streaming(&wf, &HashMap::new(), None, tx)
        .await
        .unwrap();
    assert!(result.success);

    let events: Vec<StreamEvent> = rx.try_iter().collect();
    let output_lines: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::StepOutput { line, .. } => Some(line.as_str()),
            _ => None,
        })
        .collect();
    // PTY or piped — all 3 lines must arrive.
    assert_eq!(output_lines.len(), 3, "expected 3 output lines; got: {output_lines:?}");
}

#[tokio::test]
async fn step_result_captures_output_lines() {
    let _g1 = EnvGuard::set("CLAUDECODE", "1"); // use piped path for predictable output
    let _g2 = EnvGuard::unset("CURSOR_CLI");

    let wf = make_workflow(vec![make_step("printer", "printf 'hello\nworld\n'")]);
    let result = execute_workflow(&wf, &HashMap::new(), ExecMode::Foreground, None)
        .await
        .unwrap();
    assert!(result.success);
    let step = &result.steps[0];
    assert!(
        step.output_lines.iter().any(|l| l.contains("hello")),
        "output_lines must contain 'hello'; got: {:?}", step.output_lines
    );
    assert!(
        step.output_lines.iter().any(|l| l.contains("world")),
        "output_lines must contain 'world'; got: {:?}", step.output_lines
    );
}

#[tokio::test]
async fn job_log_includes_step_output() {
    let _g1 = EnvGuard::set("CLAUDECODE", "1"); // piped path for predictable output
    let _g2 = EnvGuard::unset("CURSOR_CLI");

    let dir = tempfile::tempdir().unwrap();
    let wf = make_workflow(vec![make_step("printer", "echo 'log-line'")]);
    let result = execute_workflow(
        &wf,
        &HashMap::new(),
        ExecMode::Foreground,
        Some(dir.path().to_path_buf()),
    )
    .await
    .unwrap();
    assert!(result.success);

    // Find the job JSON.
    let json_path = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .expect("job JSON not found")
        .path();

    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(json_path).unwrap()).unwrap();
    let output = json["steps"][0]["output"].as_array().expect("output array");
    assert!(
        output.iter().any(|v| v.as_str().is_some_and(|s| s.contains("log-line"))),
        "job JSON output array must contain 'log-line'; got: {output:?}"
    );
}
