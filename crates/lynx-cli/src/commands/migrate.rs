use anyhow::Result;
use clap::Args;

use lynx_config::{config_path, load, save};
use lynx_config::migrate::{dry_run, migrate};
use lynx_config::snapshot::create as snapshot;

#[derive(Args)]
pub struct MigrateArgs {
    /// Show what would change without applying
    #[arg(long)]
    pub dry_run: bool,
}

pub async fn run(args: MigrateArgs) -> Result<()> {
    let mut cfg = load()?;

    if args.dry_run {
        let changes = dry_run(&cfg);
        for change in &changes {
            println!("{change}");
        }
        return Ok(());
    }

    // Snapshot before migrating.
    let path = config_path();
    let config_dir = path.parent().unwrap_or(&path);
    snapshot(config_dir, "pre-migrate")?;

    let before = cfg.schema_version;
    migrate(&mut cfg)?;
    let after = cfg.schema_version;

    if before == after {
        println!("config already at current schema version (v{after}) — nothing to do");
    } else {
        save(&cfg)?;
        println!("migrated config from v{before} to v{after}");
    }

    Ok(())
}
