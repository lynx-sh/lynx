use anyhow::Result;
use clap::{Args, Subcommand};
use lynx_core::paths::taps_config_path;
use lynx_registry::tap::{
    add_tap, load_taps, remove_tap, resolve_tap_url, save_taps,
};

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub struct TapArgs {
    #[command(subcommand)]
    pub command: TapCommand,
}

#[derive(Subcommand)]
pub enum TapCommand {
    /// List all configured taps
    List,
    /// Add a community tap (GitHub shorthand or full URL)
    Add {
        /// Tap source: "user/repo" or full URL to index.toml
        source: String,
    },
    /// Remove a tap
    Remove {
        /// Tap name to remove
        name: String,
    },
    /// Refresh all tap indexes
    Update,
    /// Catch unknown subcommands for friendly error
    #[command(external_subcommand)]
    Other(Vec<String>),
}

pub async fn run(args: TapArgs) -> Result<()> {
    match args.command {
        TapCommand::List => cmd_list(),
        TapCommand::Add { source } => cmd_add(&source),
        TapCommand::Remove { name } => cmd_remove(&name),
        TapCommand::Update => cmd_update(),
        TapCommand::Other(args) => {
            anyhow::bail!("unknown tap command '{}' — run `lx tap` for help", args.first().map(|s| s.as_str()).unwrap_or(""))
        }
    }
}

fn cmd_list() -> Result<()> {
    let path = taps_config_path();
    let list = load_taps(&path)?;

    println!("{:<20} {:<10} URL", "NAME", "TRUST");
    println!("{}", "-".repeat(70));
    for tap in &list.taps {
        println!(
            "{} {:<18} {:<10} {}",
            tap.trust.badge(),
            tap.name,
            tap.trust.label(),
            tap.url
        );
    }
    Ok(())
}

fn cmd_add(source: &str) -> Result<()> {
    let path = taps_config_path();
    let mut list = load_taps(&path)?;

    // Derive name from source: "user/repo" → "user/repo", full URL → last two path segments.
    let name = if source.starts_with("http") {
        source
            .trim_end_matches('/')
            .rsplit('/')
            .take(2)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("/")
    } else {
        source.to_string()
    };

    let url = resolve_tap_url(source);
    add_tap(&mut list, &name, &url)?;
    save_taps(&list, &path)?;

    println!("{} added tap '{}' ({})", "○", name, url);
    println!("  run `lx tap update` to fetch the index");
    Ok(())
}

fn cmd_remove(name: &str) -> Result<()> {
    let path = taps_config_path();
    let mut list = load_taps(&path)?;
    remove_tap(&mut list, name)?;
    save_taps(&list, &path)?;
    println!("removed tap '{name}'");
    Ok(())
}

fn cmd_update() -> Result<()> {
    let path = taps_config_path();
    let list = load_taps(&path)?;

    let mut failures = 0u32;
    let total = list.taps.len();

    for tap in &list.taps {
        print!("{} {} ... ", tap.trust.badge(), tap.name);
        match ureq::get(&tap.url).call() {
            Ok(resp) if resp.status() < 400 => {
                let cache_dir = lynx_core::paths::registry_cache_dir();
                if let Err(e) = std::fs::create_dir_all(&cache_dir) {
                    println!("failed (cannot create cache dir: {e})");
                    failures += 1;
                    continue;
                }
                let cache_file = cache_dir.join(format!("{}.toml", tap.name.replace('/', "_")));
                match resp.into_string() {
                    Ok(body) => {
                        if let Err(e) = std::fs::write(&cache_file, &body) {
                            println!("failed (cannot write cache: {e})");
                            failures += 1;
                        } else {
                            println!("ok");
                        }
                    }
                    Err(e) => {
                        println!("failed (cannot read response: {e})");
                        failures += 1;
                    }
                }
            }
            Ok(resp) => {
                println!("error (status {})", resp.status());
                failures += 1;
            }
            Err(e) => {
                println!("failed ({e})");
                failures += 1;
            }
        }
    }

    if failures > 0 && failures == total as u32 {
        anyhow::bail!("all {total} tap(s) failed to update");
    } else if failures > 0 {
        eprintln!("warning: {failures}/{total} tap(s) failed to update");
    }
    Ok(())
}
