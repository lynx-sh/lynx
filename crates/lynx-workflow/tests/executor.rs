use std::collections::HashMap;

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
    let wf = make_workflow(vec![make_step("build", "echo 'Finished release' >&2")]);
    let (tx, rx) = std::sync::mpsc::channel();
    let result = execute_workflow_streaming(&wf, &HashMap::new(), None, tx)
        .await
        .unwrap();
    assert!(result.success);

    let events: Vec<StreamEvent> = rx.try_iter().collect();
    let stderr_output: Vec<&StreamEvent> = events
        .iter()
        .filter(|e| {
            matches!(
                e,
                StreamEvent::StepOutput {
                    is_stderr: true,
                    ..
                }
            )
        })
        .collect();
    assert!(
        !stderr_output.is_empty(),
        "stderr output should be captured; events: {events:?}"
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
