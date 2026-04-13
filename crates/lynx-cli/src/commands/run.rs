use anyhow::Result;
use clap::Args;
use lynx_core::error::LynxError;
use std::collections::HashMap;

#[derive(Args)]
pub struct RunArgs {
    /// Workflow name (or 'list' to show available workflows)
    pub workflow: Option<String>,

    /// Parameters as key=value pairs
    #[arg(trailing_var_arg = true)]
    pub params: Vec<String>,

    /// Run in background immediately
    #[arg(long)]
    pub bg: bool,

    /// Show what would run without executing
    #[arg(long)]
    pub dry_run: bool,

    /// Skip all confirmation prompts
    #[arg(long)]
    pub yes: bool,
}

pub async fn run(args: RunArgs) -> Result<()> {
    // Bare `lx run` — show help
    let workflow_name = match args.workflow {
        Some(name) => name,
        None => {
            print_run_help();
            return Ok(());
        }
    };

    // Handle 'lx run list' and 'lx run examples'
    if workflow_name == "list" {
        return cmd_list();
    }
    if workflow_name == "examples" {
        crate::commands::examples::print_workflow_examples();
        return Ok(());
    }
    if workflow_name == "help" {
        print_run_help();
        return Ok(());
    }

    // Load workflow
    let wf = lynx_workflow::store::load_workflow(&workflow_name)?;

    // Parse key=value params
    let mut provided = HashMap::new();
    for param in &args.params {
        if let Some((k, v)) = param.split_once('=') {
            provided.insert(k.to_string(), v.to_string());
        } else {
            return Err(LynxError::Workflow(format!(
                "invalid param format '{param}' — expected key=value"
            ))
            .into());
        }
    }

    // Resolve params
    let params = lynx_workflow::params::resolve_params(&wf.workflow.params, &provided)?;

    // Dry run
    if args.dry_run {
        println!("Workflow: {}", wf.workflow.name);
        if !wf.workflow.description.is_empty() {
            println!("  {}", wf.workflow.description);
        }
        println!();
        for (i, step) in wf.steps.iter().enumerate() {
            let run_str = lynx_workflow::params::expand_template(&step.run, &params);
            println!("  Step {}: {} ({:?})", i + 1, step.name, step.runner);
            println!("    run: {run_str}");
            if let Some(ref g) = step.group {
                println!("    group: {g}");
            }
            if !step.depends_on.is_empty() {
                println!("    depends_on: {}", step.depends_on.join(", "));
            }
        }
        return Ok(());
    }

    let log_dir = lynx_core::paths::jobs_dir();
    std::fs::create_dir_all(&log_dir)?;

    // Background mode: use the old non-streaming executor.
    if args.bg {
        println!("Running workflow '{}' in background...", wf.workflow.name);
        let result = lynx_workflow::executor::execute_workflow(
            &wf,
            &params,
            lynx_workflow::executor::ExecMode::Background,
            Some(log_dir),
        )
        .await?;
        println!("  Job ID: {}", result.job_id);
        return Ok(());
    }

    // Interactive TUI mode: stream output in real time.
    let step_names: Vec<String> = wf.steps.iter().map(|s| s.name.clone()).collect();

    if lynx_tui::workflow::should_use_tui() {
        let (exec_tx, exec_rx) = std::sync::mpsc::channel();
        let (tui_tx, tui_rx) = std::sync::mpsc::channel();

        // Bridge executor events → TUI events on a background thread.
        std::thread::spawn(move || {
            while let Ok(ev) = exec_rx.recv() {
                let tui_ev = map_stream_to_tui(ev);
                if tui_tx.send(tui_ev).is_err() {
                    break;
                }
            }
        });

        // Spawn the executor on a background task.
        let wf_clone = wf.clone();
        let params_clone = params.clone();
        let ld = Some(log_dir.clone());
        let exec_handle = tokio::spawn(async move {
            lynx_workflow::executor::execute_workflow_streaming(
                &wf_clone,
                &params_clone,
                ld,
                exec_tx,
            )
            .await
        });

        // Run TUI (blocks until user exits).
        let tui_colors = super::tui_colors();
        let action = lynx_tui::workflow::run_workflow_tui(
            &wf.workflow.name,
            &step_names,
            tui_rx,
            &tui_colors,
        )?;

        match action {
            lynx_tui::workflow::WorkflowAction::Completed => {
                // Workflow finished, user pressed q — print summary.
                if let Ok(Ok(result)) = exec_handle.await {
                    print_summary(&result);
                }
            }
            lynx_tui::workflow::WorkflowAction::Background => {
                println!("Workflow moved to background. Check status with: lx jobs");
            }
            lynx_tui::workflow::WorkflowAction::Stopped => {
                println!("Workflow stopped.");
                exec_handle.abort();
            }
        }
    } else {
        // Non-interactive fallback: stream to stdout.
        let (tx, rx) = std::sync::mpsc::channel();

        let wf_clone = wf.clone();
        let params_clone = params.clone();
        let ld = Some(log_dir.clone());
        let exec_handle = tokio::spawn(async move {
            lynx_workflow::executor::execute_workflow_streaming(&wf_clone, &params_clone, ld, tx)
                .await
        });

        // Print events as they arrive.
        println!("Running workflow '{}'...\n", wf.workflow.name);
        loop {
            match rx.recv() {
                Ok(lynx_workflow::executor::StreamEvent::StepStarted { name }) => {
                    println!("\u{25cf} {name}");
                }
                Ok(lynx_workflow::executor::StreamEvent::StepOutput { name, line, .. }) => {
                    println!("  [{name}] {line}");
                }
                Ok(lynx_workflow::executor::StreamEvent::StepFinished {
                    name,
                    status,
                    duration_ms,
                }) => {
                    let icon = match status {
                        lynx_workflow::executor::StepStatus::Passed => "\u{2713}",
                        lynx_workflow::executor::StepStatus::Failed => "\u{2717}",
                        lynx_workflow::executor::StepStatus::Skipped => "\u{2014}",
                        lynx_workflow::executor::StepStatus::TimedOut => "\u{23f0}",
                    };
                    println!("  {icon} {name} ({duration_ms}ms)");
                }
                Ok(lynx_workflow::executor::StreamEvent::Done {
                    success,
                    duration_ms,
                }) => {
                    println!();
                    if success {
                        println!("\u{2713} Workflow completed ({duration_ms}ms)");
                    } else {
                        println!("\u{2717} Workflow failed ({duration_ms}ms)");
                    }
                    break;
                }
                Err(_) => break,
            }
        }
        let _ = exec_handle.await;
    }

    Ok(())
}

struct WorkflowListEntry {
    name: String,
    description: String,
}

impl lynx_tui::ListItem for WorkflowListEntry {
    fn title(&self) -> &str {
        &self.name
    }
    fn subtitle(&self) -> String {
        self.description.clone()
    }
    fn detail(&self) -> String {
        format!("{}\n\nRun: lx run {}", self.description, self.name)
    }
    fn category(&self) -> Option<&str> {
        Some("workflow")
    }
}

/// Map executor StreamEvent to TUI WorkflowEvent.
fn map_stream_to_tui(
    ev: lynx_workflow::executor::StreamEvent,
) -> lynx_tui::workflow::WorkflowEvent {
    use lynx_tui::workflow::{WorkflowEvent, WorkflowStepStatus};
    use lynx_workflow::executor::{StepStatus, StreamEvent};

    match ev {
        StreamEvent::StepStarted { name } => WorkflowEvent::StepStarted { name },
        StreamEvent::StepOutput {
            name,
            line,
            is_stderr,
        } => WorkflowEvent::StepOutput {
            name,
            line,
            is_stderr,
        },
        StreamEvent::StepFinished {
            name,
            status,
            duration_ms,
        } => {
            let tui_status = match status {
                StepStatus::Passed => WorkflowStepStatus::Passed,
                StepStatus::Failed => WorkflowStepStatus::Failed,
                StepStatus::Skipped => WorkflowStepStatus::Skipped,
                StepStatus::TimedOut => WorkflowStepStatus::TimedOut,
            };
            WorkflowEvent::StepFinished {
                name,
                status: tui_status,
                duration_ms,
            }
        }
        StreamEvent::Done {
            success,
            duration_ms,
        } => WorkflowEvent::Done {
            success,
            duration_ms,
        },
    }
}

fn print_summary(result: &lynx_workflow::executor::JobResult) {
    for step in &result.steps {
        let status = match step.status {
            lynx_workflow::executor::StepStatus::Passed => "\u{2713}",
            lynx_workflow::executor::StepStatus::Failed => "\u{2717}",
            lynx_workflow::executor::StepStatus::Skipped => "\u{2014}",
            lynx_workflow::executor::StepStatus::TimedOut => "\u{23f0}",
        };
        println!("  {status} {}  ({}ms)", step.name, step.duration_ms);
    }
    println!();
    if result.success {
        println!("\u{2713} Workflow completed ({}ms)", result.duration_ms);
    } else {
        println!("\u{2717} Workflow failed ({}ms)", result.duration_ms);
        println!("  Job ID: {}", result.job_id);
    }
}

fn print_run_help() {
    println!(
        r#"
  lx run — workflow runner
  ────────────────────────

  Workflows let you save a sequence of commands as a reusable recipe.
  Instead of typing the same build/test/deploy steps every time, you
  write them once in a TOML file and run them with one command.

  Quick start:
    1. Create a workflow file:
       ~/.config/lynx/workflows/check.toml

    2. Add your steps:

       [workflow]
       name = "check"
       description = "Lint and test my project"

       [[step]]
       name = "lint"
       run = "cargo clippy --all"

       [[step]]
       name = "test"
       run = "cargo nextest run --all"

    3. Run it:
       lx run check

  That's it! Lynx runs each step in order and shows you the results.

  Going further:
    Parallel steps    give steps the same group name
    Dependencies      depends_on = ["step1", "step2"]
    Parameters        lx run deploy env=prod
    Dry run           lx run deploy --dry-run
    Background        lx run deploy --bg
    Skip prompts      lx run deploy --yes
    Different runner  runner = "bash", "python", "node", etc.
    Retry on fail     on_fail = "retry" + retry_count = 3
    Timeout           timeout_sec = 300
    Confirm first     confirm = true

  Commands:
    lx run <name>          run a workflow
    lx run <name> --help   see workflow-specific params
    lx run list            browse available workflows
    lx run examples        full examples with TOML snippets

  Workflow files live in: ~/.config/lynx/workflows/
"#
    );
}

fn cmd_list() -> Result<()> {
    let entries = lynx_workflow::store::list_workflows()?;
    if entries.is_empty() {
        println!("No workflows found.");
        println!("Create workflow files in ~/.config/lynx/workflows/");
        return Ok(());
    }

    let items: Vec<WorkflowListEntry> = entries
        .iter()
        .map(|e| WorkflowListEntry {
            name: e.name.clone(),
            description: e.description.clone(),
        })
        .collect();

    if let Some(idx) = lynx_tui::show(&items, "Workflows", &super::tui_colors())? {
        println!("  Run: lx run {}", items[idx].name);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_list_entry_trait() {
        use lynx_tui::ListItem;
        let entry = WorkflowListEntry {
            name: "deploy".to_string(),
            description: "Deploy to prod".to_string(),
        };
        assert_eq!(entry.title(), "deploy");
        assert_eq!(entry.subtitle(), "Deploy to prod");
        assert!(entry.detail().contains("lx run deploy"));
        assert_eq!(entry.category(), Some("workflow"));
    }

    #[test]
    fn map_stream_to_tui_step_started() {
        use lynx_workflow::executor::StreamEvent;
        let ev = StreamEvent::StepStarted {
            name: "lint".to_string(),
        };
        let tui_ev = map_stream_to_tui(ev);
        assert!(
            matches!(tui_ev, lynx_tui::workflow::WorkflowEvent::StepStarted { name } if name == "lint")
        );
    }

    #[test]
    fn map_stream_to_tui_step_output() {
        use lynx_workflow::executor::StreamEvent;
        let ev = StreamEvent::StepOutput {
            name: "test".to_string(),
            line: "ok".to_string(),
            is_stderr: false,
        };
        let tui_ev = map_stream_to_tui(ev);
        assert!(matches!(
            tui_ev,
            lynx_tui::workflow::WorkflowEvent::StepOutput { .. }
        ));
    }

    #[test]
    fn map_stream_to_tui_step_finished_all_statuses() {
        use lynx_tui::workflow::{WorkflowEvent, WorkflowStepStatus};
        use lynx_workflow::executor::{StepStatus, StreamEvent};

        for (input, expected) in [
            (StepStatus::Passed, WorkflowStepStatus::Passed),
            (StepStatus::Failed, WorkflowStepStatus::Failed),
            (StepStatus::Skipped, WorkflowStepStatus::Skipped),
            (StepStatus::TimedOut, WorkflowStepStatus::TimedOut),
        ] {
            let ev = StreamEvent::StepFinished {
                name: "s".to_string(),
                status: input,
                duration_ms: 100,
            };
            let tui_ev = map_stream_to_tui(ev);
            match tui_ev {
                WorkflowEvent::StepFinished { status, .. } => assert_eq!(status, expected),
                _ => panic!("expected StepFinished"),
            }
        }
    }

    #[test]
    fn map_stream_to_tui_done() {
        use lynx_workflow::executor::StreamEvent;
        let ev = StreamEvent::Done {
            success: true,
            duration_ms: 500,
        };
        let tui_ev = map_stream_to_tui(ev);
        assert!(matches!(
            tui_ev,
            lynx_tui::workflow::WorkflowEvent::Done {
                success: true,
                duration_ms: 500
            }
        ));
    }

    #[test]
    fn parse_key_value_params_valid() {
        let params = vec!["env=staging".to_string(), "verbose=true".to_string()];
        let mut provided = HashMap::new();
        for param in &params {
            if let Some((k, v)) = param.split_once('=') {
                provided.insert(k.to_string(), v.to_string());
            }
        }
        assert_eq!(provided.get("env").unwrap(), "staging");
        assert_eq!(provided.get("verbose").unwrap(), "true");
    }

    #[test]
    fn parse_key_value_params_no_equals_not_inserted() {
        let params = vec!["no-equals-here".to_string()];
        let mut provided = HashMap::new();
        for param in &params {
            if let Some((k, v)) = param.split_once('=') {
                provided.insert(k.to_string(), v.to_string());
            }
        }
        assert!(provided.is_empty());
    }

    #[test]
    fn parse_key_value_params_value_with_equals() {
        // "key=value=extra" should split at first =
        let param = "url=https://example.com?a=b";
        let (k, v) = param.split_once('=').unwrap();
        assert_eq!(k, "url");
        assert_eq!(v, "https://example.com?a=b");
    }

    #[test]
    fn run_args_defaults() {
        use clap::Parser;
        #[derive(Parser)]
        struct W {
            #[command(flatten)]
            args: RunArgs,
        }
        let w = W::parse_from(["test"]);
        assert!(w.args.workflow.is_none());
        assert!(!w.args.bg);
        assert!(!w.args.dry_run);
        assert!(!w.args.yes);
    }

    #[test]
    fn print_run_help_does_not_panic() {
        print_run_help();
    }
}
