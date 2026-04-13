use anyhow::{bail, Result};
use clap::Args;
use std::collections::HashMap;

#[derive(Args)]
pub struct RunArgs {
    /// Workflow name (or 'list' to show available workflows)
    pub workflow: String,

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
    // Handle 'lx run list'
    if args.workflow == "list" {
        return cmd_list();
    }

    // Load workflow
    let wf = lynx_workflow::store::load_workflow(&args.workflow)?;

    // Parse key=value params
    let mut provided = HashMap::new();
    for param in &args.params {
        if let Some((k, v)) = param.split_once('=') {
            provided.insert(k.to_string(), v.to_string());
        } else {
            bail!("invalid param format '{}' — expected key=value", param);
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
