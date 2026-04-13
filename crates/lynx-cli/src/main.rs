mod bus;
mod cli;
mod commands;
mod error_display;

use anyhow::Result;
use clap::{CommandFactory, Parser, error::ErrorKind};
use cli::Cli;

#[tokio::main]
async fn main() {
    // Use try_parse so we can intercept "missing subcommand" errors and redirect
    // help to stdout with exit 0, rather than clap's default (stderr, exit 2).
    // This makes `lx`, `lx theme`, `lx plugin` etc. behave as informational
    // commands rather than parse errors.
    let result = match Cli::try_parse() {
        Ok(cli) => commands::dispatch(cli).await,
        Err(e) if matches!(
            e.kind(),
            ErrorKind::MissingSubcommand | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
        ) => {
            let args: Vec<String> = std::env::args().skip(1).collect();
            if args.is_empty() || args.first().map(|s| s.as_str()) == Some("help") {
                // Bare `lx` or `lx help` → interactive help browser.
                commands::help::show_interactive_help()
            } else {
                // `lx theme` etc. → show that subcommand's help inline.
                let root = Cli::command();
                print_subcommand_help(root, &args)
            }
        }
        Err(e) if matches!(e.kind(), ErrorKind::InvalidSubcommand) => {
            // Unknown top-level subcommand — render through our error system
            // so users see the styled red error + hint, not clap's generic stderr.
            let args: Vec<String> = std::env::args().skip(1).collect();
            let cmd = args.first().map(|s| s.as_str()).unwrap_or("?");
            Err(lynx_core::error::LynxError::unknown_command(cmd, "").into())
        }
        Err(e) => {
            // All other clap errors (unknown flag, bad arg value, etc.) go to
            // stderr as normal — these ARE errors the user should fix.
            e.exit()
        }
    };

    // Print errors to STDOUT so they're visible even when the shell precmd hook
    // re-renders the prompt (which can overwrite stderr output).
    if let Err(e) = result {
        error_display::render_error(&e);
        std::process::exit(1);
    }
}

/// Walk `args` depth-first into `cmd`'s subcommands, printing help for the
/// deepest match found. Falls back to `cmd`'s own help.
fn print_subcommand_help(mut cmd: clap::Command, args: &[String]) -> Result<()> {
    for arg in args {
        if let Some(sub) = cmd.find_subcommand(arg).cloned() {
            cmd = sub;
        }
    }
    cmd.print_help()?;
    println!();
    Ok(())
}
