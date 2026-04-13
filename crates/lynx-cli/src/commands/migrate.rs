use anyhow::Result;
use clap::Args;

use lynx_config::load;
use lynx_config::migrate::{dry_run, migrate};
use lynx_config::snapshot::mutate_config_transaction;

#[derive(Args)]
pub struct MigrateArgs {
    /// Show what would change without applying
    #[arg(long)]
    pub dry_run: bool,
}

pub fn run(args: MigrateArgs) -> Result<()> {
    let mut cfg = load()?;

    if args.dry_run {
        let changes = dry_run(&cfg);
        for change in &changes {
            println!("{change}");
        }
        return Ok(());
    }

    let before = cfg.schema_version;
    migrate(&mut cfg)?;
    let after = cfg.schema_version;

    if before == after {
        println!("config already at current schema version (v{after}) — nothing to do");
    } else {
        mutate_config_transaction("pre-migrate", |config| {
            *config = cfg.clone();
            Ok(())
        })?;
        println!("migrated config from v{before} to v{after}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_args_defaults() {
        use clap::Parser;
        #[derive(Parser)]
        struct W {
            #[command(flatten)]
            args: MigrateArgs,
        }
        let w = W::parse_from(["test"]);
        assert!(!w.args.dry_run);
    }

    #[test]
    fn migrate_args_dry_run() {
        use clap::Parser;
        #[derive(Parser)]
        struct W {
            #[command(flatten)]
            args: MigrateArgs,
        }
        let w = W::parse_from(["test", "--dry-run"]);
        assert!(w.args.dry_run);
    }
}
