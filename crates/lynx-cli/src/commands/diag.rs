use anyhow::Result;
use clap::{Args, Subcommand};
use lynx_core::diag;
/// `lx diag` — view and manage the Lynx diagnostic log.
///
/// Background operations (init, plugin load) write errors here instead of
/// printing to stderr (which would corrupt the terminal during shell startup).
/// Use `lx diag` to see what Lynx has logged. Use `lx doctor` for a full
/// health check with actionable fixes.
use lynx_core::error::LynxError;

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
    /// Catch unknown subcommands for friendly error
    #[command(external_subcommand)]
    Other(Vec<String>),
}

pub fn run(args: DiagArgs) -> Result<()> {
    match args.command {
        Some(DiagCommand::Clear) => {
            diag::clear()?;
            println!("Diagnostic log cleared.");
        }
        Some(DiagCommand::Path) => {
            println!("{}", diag::log_path().display());
        }
        Some(DiagCommand::Other(args)) => {
            return Err(LynxError::unknown_command(
                super::unknown_subcmd_name(&args),
                "diag",
            )
            .into())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diag_args_default_lines_is_50() {
        // Verify the struct default via clap parsing
        use clap::Parser;
        #[derive(Parser)]
        struct Wrapper {
            #[command(flatten)]
            args: DiagArgs,
        }
        let w = Wrapper::parse_from(["test"]);
        assert_eq!(w.args.lines, 50);
        assert!(w.args.command.is_none());
    }

    #[test]
    fn diag_args_custom_lines() {
        use clap::Parser;
        #[derive(Parser)]
        struct Wrapper {
            #[command(flatten)]
            args: DiagArgs,
        }
        let w = Wrapper::parse_from(["test", "-n", "10"]);
        assert_eq!(w.args.lines, 10);
    }

    #[test]
    fn diag_clear_subcommand_parses() {
        use clap::Parser;
        #[derive(Parser)]
        struct Wrapper {
            #[command(flatten)]
            args: DiagArgs,
        }
        let w = Wrapper::parse_from(["test", "clear"]);
        assert!(matches!(w.args.command, Some(DiagCommand::Clear)));
    }

    #[test]
    fn diag_path_subcommand_parses() {
        use clap::Parser;
        #[derive(Parser)]
        struct Wrapper {
            #[command(flatten)]
            args: DiagArgs,
        }
        let w = Wrapper::parse_from(["test", "path"]);
        assert!(matches!(w.args.command, Some(DiagCommand::Path)));
    }

    #[tokio::test]
    async fn diag_unknown_subcommand_returns_error() {
        let args = DiagArgs {
            command: Some(DiagCommand::Other(vec!["bogus".to_string()])),
            lines: 50,
        };
        let err = run(args).unwrap_err();
        assert!(err.to_string().contains("bogus"));
    }
}
