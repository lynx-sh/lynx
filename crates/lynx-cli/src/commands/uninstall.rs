use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Args;

use lynx_core::brand;

const LYNX_INIT_PATTERN: &str = r#"source "${HOME}/.config/lynx/shell/init.zsh""#;

#[derive(Args)]
pub struct UninstallArgs {
    /// Also remove ~/.config/lynx/ (prompts unless --purge)
    #[arg(long)]
    pub purge: bool,
    /// Skip confirmation prompts (use with --purge)
    #[arg(long)]
    pub yes: bool,
}

pub async fn run(args: UninstallArgs) -> Result<()> {
    let home = home_dir()?;

    println!("Uninstalling Lynx...");
    println!();

    // 1. Remove LYNX_SOURCE_LINE from .zshrc
    remove_from_zshrc(&home)?;

    // 2. Stop daemon (macOS: launchd unload)
    stop_daemon();

    // 3. Remove lx binary
    remove_binary(&home);

    // 4. Remove runtime dir
    if let Ok(rt) = lynx_core::runtime::runtime_dir() {
        if rt.exists() {
            let _ = std::fs::remove_dir_all(&rt);
            println!("  removed runtime dir: {rt:?}");
        }
    }

    // 5. Optionally remove config dir
    let config_dir = home.join(brand::CONFIG_DIR);
    if args.purge && config_dir.exists() {
        if !args.yes {
            eprint!(
                "Remove {} (including your themes, plugins, and snapshots)? [y/N] ",
                config_dir.display()
            );
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Config preserved at {}", config_dir.display());
                println!("Run 'rm -rf {}' to remove manually.", config_dir.display());
                return Ok(());
            }
        }
        // List user-created items before removing.
        list_user_files(&config_dir);
        std::fs::remove_dir_all(&config_dir)
            .map_err(|e| anyhow::anyhow!("failed to remove config dir: {e}"))?;
        println!("  removed config dir: {}", config_dir.display());
    } else if args.purge {
        println!("  config dir not found — nothing to remove");
    } else {
        println!(
            "  config preserved at {} (use --purge to remove)",
            config_dir.display()
        );
    }

    println!();
    println!("Lynx uninstalled. Restart your shell to complete.");
    Ok(())
}

fn remove_from_zshrc(home: &Path) -> Result<()> {
    let zshrc = home.join(".zshrc");
    if !zshrc.exists() {
        println!("  .zshrc not found — skipping");
        return Ok(());
    }

    let content = std::fs::read_to_string(&zshrc)?;
    let filtered: Vec<&str> = content
        .lines()
        .filter(|l| !l.contains(LYNX_INIT_PATTERN))
        .collect();

    if filtered.len() == content.lines().count() {
        println!("  .zshrc: no Lynx init line found — skipping");
    } else {
        let new_content = filtered.join("\n") + if content.ends_with('\n') { "\n" } else { "" };
        std::fs::write(&zshrc, new_content)?;
        println!("  removed Lynx init line from ~/.zshrc");
    }
    Ok(())
}

fn stop_daemon() {
    // macOS launchd
    #[cfg(target_os = "macos")]
    {
        let plist = format!(
            "{}/Library/LaunchAgents/com.proxikal.{}.plist",
            std::env::var("HOME").unwrap_or_default(),
            brand::DAEMON_NAME,
        );
        if std::path::Path::new(&plist).exists() {
            let _ = std::process::Command::new("launchctl")
                .args(["unload", &plist])
                .status();
            let _ = std::fs::remove_file(&plist);
            println!("  unloaded and removed launchd plist");
        }
    }

    // Linux systemd (best-effort)
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "stop", brand::DAEMON_NAME])
            .status();
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "disable", brand::DAEMON_NAME])
            .status();
    }
}

fn remove_binary(home: &Path) {
    let candidates = vec![
        home.join(".local/bin").join(brand::CLI),
        PathBuf::from(format!("/usr/local/bin/{}", brand::CLI)),
    ];
    for path in candidates {
        if path.exists() {
            if let Err(e) = std::fs::remove_file(&path) {
                eprintln!("  warn: could not remove {path:?}: {e}");
            } else {
                println!("  removed binary: {path:?}");
            }
        }
    }
}

fn list_user_files(config_dir: &Path) {
    let subdirs = ["plugins", "themes"];
    for sub in subdirs {
        let dir = config_dir.join(sub);
        if dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    println!("  will remove: {}", entry.path().display());
                }
            }
        }
    }
}

fn home_dir() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("$HOME not set"))
}
