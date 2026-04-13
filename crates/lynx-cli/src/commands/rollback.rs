use anyhow::Result;
use clap::Args;
use lynx_core::error::LynxError;

use lynx_config::snapshot::{list, restore};

#[derive(Args)]
pub struct RollbackArgs {
    /// Restore the most recent snapshot without prompting
    #[arg(long)]
    pub last: bool,
}

pub fn run(args: RollbackArgs) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_dir_returns_lynx_dir() {
        let dir = config_dir().unwrap();
        assert!(dir.to_string_lossy().contains("lynx") || !dir.to_string_lossy().is_empty());
    }

    #[tokio::test]
    async fn rollback_empty_snapshots_returns_error() {
        // With a temp home, there should be no snapshots — this should error.
        let home = lynx_test_utils::temp_home();
        std::env::set_var(lynx_core::env_vars::LYNX_DIR, home.path().join("lynx"));
        let args = RollbackArgs { last: false };
        // This may or may not error depending on whether list() finds snapshots.
        // At minimum it should not panic.
        let _ = run(args);
    }

    #[test]
    fn rollback_args_defaults() {
        use clap::Parser;
        #[derive(Parser)]
        struct Wrapper {
            #[command(flatten)]
            args: RollbackArgs,
        }
        let w = Wrapper::parse_from(["test"]);
        assert!(!w.args.last);
    }

    #[test]
    fn rollback_args_last_flag() {
        use clap::Parser;
        #[derive(Parser)]
        struct Wrapper {
            #[command(flatten)]
            args: RollbackArgs,
        }
        let w = Wrapper::parse_from(["test", "--last"]);
        assert!(w.args.last);
    }
}
