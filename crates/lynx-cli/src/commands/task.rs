use anyhow::Result;
use clap::{Args, Subcommand};

#[derive(Args)]
pub struct TaskArgs {
    #[command(subcommand)]
    pub command: TaskCommand,
}

#[derive(Subcommand)]
pub enum TaskCommand {
    /// Add a scheduled task
    Add { name: String, #[arg(long)] run: String, #[arg(long)] cron: String },
    /// List all tasks
    List,
    /// Show logs for a task
    Logs { name: String },
    /// Pause a task
    Pause { name: String },
    /// Resume a task
    Resume { name: String },
    /// Run a task immediately
    Run { name: String },
    /// Remove a task
    Remove { name: String },
}

pub async fn run(_args: TaskArgs) -> Result<()> {
    todo!("task commands")
}
