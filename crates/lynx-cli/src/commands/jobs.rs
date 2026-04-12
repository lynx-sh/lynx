use anyhow::Result;
use clap::{Args, Subcommand};

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub struct JobsArgs {
    #[command(subcommand)]
    pub command: JobsCommand,
}

#[derive(Subcommand)]
pub enum JobsCommand {
    /// List recent workflow jobs
    List,
    /// Show details for a specific job
    View {
        /// Job ID
        id: String,
    },
    /// Kill a running job
    Kill {
        /// Job ID
        id: String,
    },
    /// Show log output for a job
    Log {
        /// Job ID
        id: String,
    },
    /// Clean old job records
    Clean {
        /// Remove jobs older than N hours (default: 72)
        #[arg(long, default_value_t = 72)]
        hours: u64,
    },
}

pub async fn run(args: JobsArgs) -> Result<()> {
    match args.command {
        JobsCommand::List => cmd_list(),
        JobsCommand::View { id } => cmd_view(&id),
        JobsCommand::Kill { id } => cmd_kill(&id),
        JobsCommand::Log { id } => cmd_log(&id),
        JobsCommand::Clean { hours } => cmd_clean(hours),
    }
}

fn cmd_list() -> Result<()> {
    let entries = lynx_workflow::jobs::list_jobs()?;
    if entries.is_empty() {
        println!("No jobs found.");
        return Ok(());
    }

    println!(
        "{:<30} {:<15} {:<10} {:<12}",
        "JOB ID", "WORKFLOW", "STATUS", "DURATION"
    );
    println!("{}", "-".repeat(70));
    for e in &entries {
        let status = if e.success { "pass" } else { "fail" };
        let dur = format!("{}ms", e.duration_ms);
        println!("{:<30} {:<15} {:<10} {:<12}", e.job_id, e.workflow, status, dur);
    }
    Ok(())
}

fn cmd_view(id: &str) -> Result<()> {
    let job = lynx_workflow::jobs::get_job(id)?;
    println!("{}", serde_json::to_string_pretty(&job)?);
    Ok(())
}

fn cmd_kill(id: &str) -> Result<()> {
    lynx_workflow::jobs::kill_job(id)?;
    println!("Sent kill signal to job '{id}'.");
    Ok(())
}

fn cmd_log(id: &str) -> Result<()> {
    let content = lynx_workflow::jobs::read_job_log(id)?;
    print!("{content}");
    Ok(())
}

fn cmd_clean(hours: u64) -> Result<()> {
    let removed = lynx_workflow::jobs::clean_jobs(hours)?;
    println!("Cleaned {removed} old job record(s).");
    Ok(())
}
