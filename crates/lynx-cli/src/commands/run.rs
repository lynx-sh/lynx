use anyhow::{Result};
use lynx_core::error::LynxError;
use clap::Args;
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

    // Handle 'lx run list'
    if workflow_name == "list" {
        return cmd_list();
    }

    // Load workflow
    let wf = lynx_workflow::store::load_workflow(&workflow_name)?;

    // Parse key=value params
    let mut provided = HashMap::new();
    for param in &args.params {
        if let Some((k, v)) = param.split_once('=') {
            provided.insert(k.to_string(), v.to_string());
        } else {
            return Err(LynxError::Workflow(format!("invalid param format '{param}' — expected key=value")).into());
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
            println!(
                "  Step {}: {} ({:?})",
                i + 1,
                step.name,
                step.runner
            );
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

    // Execute
    let mode = if args.bg {
        lynx_workflow::executor::ExecMode::Background
    } else {
        lynx_workflow::executor::ExecMode::Foreground
    };

    let log_dir = lynx_core::paths::jobs_dir();
    std::fs::create_dir_all(&log_dir)?;

    println!("Running workflow '{}'...", wf.workflow.name);
    println!();

    let result =
        lynx_workflow::executor::execute_workflow(&wf, &params, mode, Some(log_dir)).await?;

    // Print summary
    println!();
    for step in &result.steps {
        let status = match step.status {
            lynx_workflow::executor::StepStatus::Passed => "\u{2713}",
            lynx_workflow::executor::StepStatus::Failed => "\u{2717}",
            lynx_workflow::executor::StepStatus::Skipped => "\u{2014}",
            lynx_workflow::executor::StepStatus::TimedOut => "\u{23F0}",
        };
        println!(
            "  {status} {}  ({}ms)",
            step.name, step.duration_ms
        );
    }

    println!();
    if result.success {
        println!("\u{2713} Workflow completed successfully ({}ms)", result.duration_ms);
    } else {
        println!("\u{2717} Workflow failed ({}ms)", result.duration_ms);
        println!("  Job ID: {}", result.job_id);
    }

    Ok(())
}

struct WorkflowListEntry {
    name: String,
    description: String,
}

impl lynx_tui::ListItem for WorkflowListEntry {
    fn title(&self) -> &str { &self.name }
    fn subtitle(&self) -> String { self.description.clone() }
    fn detail(&self) -> String {
        format!("{}\n\nRun: lx run {}", self.description, self.name)
    }
    fn category(&self) -> Option<&str> { Some("workflow") }
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
    lx examples run        full examples with TOML snippets

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

    let items: Vec<WorkflowListEntry> = entries.iter().map(|e| WorkflowListEntry {
        name: e.name.clone(),
        description: e.description.clone(),
    }).collect();

    if let Some(idx) = lynx_tui::show(&items, "Workflows", &super::tui_colors())? {
        println!("  Run: lx run {}", items[idx].name);
    }
    Ok(())
}
