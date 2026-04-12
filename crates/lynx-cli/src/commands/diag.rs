/// `lx diag` — view and manage the Lynx diagnostic log.
///
/// Background operations (init, plugin load) write errors here instead of
/// printing to stderr (which would corrupt the terminal during shell startup).
/// Use `lx diag` to see what Lynx has logged. Use `lx doctor` for a full
/// health check with actionable fixes.
use anyhow::Result;
use clap::{Args, Subcommand};
use lynx_core::diag;

#[derive(Args)]
pub struct DiagArgs {
    #[command(subcommand)]
    pub command: Option<DiagCommand>,

    /// Number of lines to show (default: 50)
    #[arg(short = 'n', long, default_value = "50")]
    pub lines: usize,
}

#[derive(Subcommand)]
pub enum DiagCommand {
    /// Clear the diagnostic log
    Clear,
    /// Show the path to the diagnostic log file
    Path,
}

pub async fn run(args: DiagArgs) -> Result<()> {
    match args.command {
        Some(DiagCommand::Clear) => {
            diag::clear()?;
            println!("Diagnostic log cleared.");
        }
        Some(DiagCommand::Path) => {
            println!("{}", diag::log_path().display());
        }
        None => {
            let lines = diag::tail(args.lines);
            if lines.is_empty() {
                println!("No diagnostic entries. Lynx is running clean.");
            } else {
                println!("Lynx diagnostic log (last {} entries):", lines.len());
                println!();
                for line in &lines {
                    println!("  {line}");
                }
                println!();
                println!("Tip: run `lx doctor` for a full health check with fixes.");
            }
        }
    }
    Ok(())
}
