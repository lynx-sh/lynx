mod bus;
mod cli;
mod commands;

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
            // Walk env args to find the deepest matching subcommand and print its
            // help to stdout. Fall back to root help if no subcommand matched.
            let args: Vec<String> = std::env::args().skip(1).collect();
            let root = Cli::command();
            print_subcommand_help(root, &args)
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
        println!("error: {e:#}");
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
