use anyhow::Result;
use clap::{CommandFactory, Parser};
use clap_complete::{generate, Shell};

#[derive(Parser)]
pub struct CompletionsArgs {
    /// Shell to generate completions for
    #[arg(value_enum)]
    pub shell: Shell,
}

pub fn run(args: CompletionsArgs) -> Result<()> {
    let mut cmd = crate::cli::Cli::command();
    generate(args.shell, &mut cmd, "lx", &mut std::io::stdout());
    Ok(())
}
