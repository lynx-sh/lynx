use anyhow::Result;
use clap::{Args, Subcommand};
use lynx_core::error::LynxError;

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
    /// Catch unknown subcommands for friendly error
    #[command(external_subcommand)]
    Other(Vec<String>),
}

pub fn run(args: JobsArgs) -> Result<()> {
    match args.command {
        JobsCommand::List => cmd_list(),
        JobsCommand::View { id } => cmd_view(&id),
        JobsCommand::Kill { id } => cmd_kill(&id),
        JobsCommand::Log { id } => cmd_log(&id),
        JobsCommand::Clean { hours } => cmd_clean(hours),
        JobsCommand::Other(args) => {
            Err(LynxError::unknown_command(super::unknown_subcmd_name(&args), "jobs").into())
        }
    }
}

struct JobListEntry {
    job_id: String,
    workflow: String,
    success: bool,
    duration_ms: u64,
}

impl lynx_tui::ListItem for JobListEntry {
    fn title(&self) -> &str {
        &self.job_id
    }
    fn subtitle(&self) -> String {
        let status = if self.success { "pass" } else { "fail" };
        format!("{} — {status}", self.workflow)
    }
    fn detail(&self) -> String {
        format!(
            "Workflow: {}\nStatus: {}\nDuration: {}ms",
            self.workflow,
            if self.success { "pass" } else { "fail" },
            self.duration_ms
        )
    }
    fn is_active(&self) -> bool {
        self.success
    }
}

fn cmd_list() -> Result<()> {
    let entries = lynx_workflow::jobs::list_jobs()?;
    if entries.is_empty() {
        println!("No jobs found.");
        return Ok(());
    }

    let items: Vec<JobListEntry> = entries
        .iter()
        .map(|e| JobListEntry {
            job_id: e.job_id.clone(),
            workflow: e.workflow.clone(),
            success: e.success,
            duration_ms: e.duration_ms,
        })
        .collect();

    lynx_tui::show(&items, "Jobs", &super::tui_colors())?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_list_entry_title_is_job_id() {
        use lynx_tui::ListItem;
        let entry = JobListEntry {
            job_id: "deploy-123".to_string(),
            workflow: "deploy".to_string(),
            success: true,
            duration_ms: 1500,
        };
        assert_eq!(entry.title(), "deploy-123");
    }

    #[test]
    fn job_list_entry_subtitle_shows_status() {
        use lynx_tui::ListItem;
        let pass = JobListEntry {
            job_id: "j1".to_string(),
            workflow: "build".to_string(),
            success: true,
            duration_ms: 100,
        };
        assert!(pass.subtitle().contains("pass"));

        let fail = JobListEntry {
            job_id: "j2".to_string(),
            workflow: "test".to_string(),
            success: false,
            duration_ms: 200,
        };
        assert!(fail.subtitle().contains("fail"));
    }

    #[test]
    fn job_list_entry_detail_includes_duration() {
        use lynx_tui::ListItem;
        let entry = JobListEntry {
            job_id: "j1".to_string(),
            workflow: "deploy".to_string(),
            success: true,
            duration_ms: 5000,
        };
        let detail = entry.detail();
        assert!(
            detail.contains("5000"),
            "detail should contain duration: {detail}"
        );
        assert!(detail.contains("deploy"));
    }

    #[test]
    fn job_list_entry_is_active_reflects_success() {
        use lynx_tui::ListItem;
        let pass = JobListEntry {
            job_id: "j1".to_string(),
            workflow: "x".to_string(),
            success: true,
            duration_ms: 0,
        };
        assert!(pass.is_active());

        let fail = JobListEntry {
            job_id: "j2".to_string(),
            workflow: "x".to_string(),
            success: false,
            duration_ms: 0,
        };
        assert!(!fail.is_active());
    }

    #[tokio::test]
    async fn jobs_unknown_subcommand_errors() {
        let args = JobsArgs {
            command: JobsCommand::Other(vec!["bogus".to_string()]),
        };
        let err = run(args).unwrap_err();
        assert!(err.to_string().contains("bogus"));
    }
}
