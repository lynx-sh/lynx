use anyhow::Result;
use lynx_core::error::LynxError;
use clap::Args;

use lynx_config::snapshot::{list, restore};

#[derive(Args)]
pub struct RollbackArgs {
    /// Restore the most recent snapshot without prompting
    #[arg(long)]
    pub last: bool,
}

pub async fn run(args: RollbackArgs) -> Result<()> {
    let snaps = list()?;

    if snaps.is_empty() {
        return Err(LynxError::Config("no snapshots found — nothing to rollback".into()).into());
    }

    if args.last {
        let (label, path) = &snaps[0];
        let config_dir = config_dir()?;
        restore(path, &config_dir)?;
        println!("restored snapshot: {label}");
        return Ok(());
    }

    // List and prompt.
    println!("Available snapshots (newest first):");
    for (i, (label, _)) in snaps.iter().enumerate().take(10) {
        println!("  [{}] {}", i + 1, label);
    }

    // In a non-interactive context just list and exit.
    // A real TUI prompt would go here; for now print the --last hint.
    println!();
    println!("Run with --last to restore the most recent, or specify a snapshot.");
    println!("  lx rollback --last");
    Ok(())
}

fn config_dir() -> Result<std::path::PathBuf> {
    Ok(lynx_core::paths::lynx_dir())
}
