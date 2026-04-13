use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Args;

use lynx_core::brand;

use lynx_core::brand::ZSHRC_INIT_LINE;

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
            print!(
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
            .map_err(|e| anyhow::Error::from(lynx_core::error::LynxError::Io {
                message: format!("failed to remove config dir: {e}"),
                path: config_dir.clone(),
                fix: "check permissions or remove manually".into(),
            }))?;
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
        .filter(|l| !l.contains(ZSHRC_INIT_LINE))
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
            "{}/Library/LaunchAgents/{}.plist",
            std::env::var(lynx_core::env_vars::HOME).unwrap_or_default(),
            lynx_core::brand::LAUNCHD_LABEL,
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
            .args(["--user", "stop", lynx_core::brand::SYSTEMD_SERVICE])
            .status();
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "disable", lynx_core::brand::SYSTEMD_SERVICE])
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
                println!("  warn: could not remove {path:?}: {e}");
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
        .ok_or_else(|| anyhow::Error::from(lynx_core::error::LynxError::Shell("$HOME not set".into())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_dir_returns_value_when_set() {
        // HOME should be set in the test environment
        let dir = home_dir().unwrap();
        assert!(!dir.as_os_str().is_empty());
    }

    #[test]
    fn remove_from_zshrc_no_file_is_ok() {
        let tmp = tempfile::tempdir().unwrap();
        // No .zshrc exists — should succeed silently
        let result = remove_from_zshrc(tmp.path());
        assert!(result.is_ok());
    }

    #[test]
    fn remove_from_zshrc_removes_init_line() {
        let tmp = tempfile::tempdir().unwrap();
        let zshrc = tmp.path().join(".zshrc");
        let content = format!(
            "# my config\n{}\nexport FOO=bar\n",
            ZSHRC_INIT_LINE
        );
        std::fs::write(&zshrc, &content).unwrap();

        remove_from_zshrc(tmp.path()).unwrap();

        let after = std::fs::read_to_string(&zshrc).unwrap();
        assert!(!after.contains(ZSHRC_INIT_LINE));
        assert!(after.contains("my config"));
        assert!(after.contains("FOO=bar"));
    }

    #[test]
    fn remove_from_zshrc_no_init_line_is_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let zshrc = tmp.path().join(".zshrc");
        let content = "export PATH=/usr/bin\n";
        std::fs::write(&zshrc, content).unwrap();

        remove_from_zshrc(tmp.path()).unwrap();

        let after = std::fs::read_to_string(&zshrc).unwrap();
        assert_eq!(after, content);
    }

    #[test]
    fn remove_from_zshrc_preserves_trailing_newline() {
        let tmp = tempfile::tempdir().unwrap();
        let zshrc = tmp.path().join(".zshrc");
        let content = format!("line1\n{}\nline3\n", ZSHRC_INIT_LINE);
        std::fs::write(&zshrc, &content).unwrap();

        remove_from_zshrc(tmp.path()).unwrap();

        let after = std::fs::read_to_string(&zshrc).unwrap();
        assert!(after.ends_with('\n'));
    }

    #[test]
    fn list_user_files_does_not_panic_on_missing_dir() {
        let tmp = tempfile::tempdir().unwrap();
        // Should not panic even if plugins/themes dirs don't exist
        list_user_files(tmp.path());
    }

    #[test]
    fn list_user_files_lists_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let plugins = tmp.path().join("plugins");
        std::fs::create_dir_all(&plugins).unwrap();
        std::fs::write(plugins.join("git"), "").unwrap();
        // Should not panic
        list_user_files(tmp.path());
    }

    #[test]
    fn uninstall_args_defaults() {
        use clap::Parser;
        #[derive(Parser)]
        struct W {
            #[command(flatten)]
            args: UninstallArgs,
        }
        let w = W::parse_from(["test"]);
        assert!(!w.args.purge);
        assert!(!w.args.yes);
    }

    #[test]
    fn uninstall_args_purge_yes() {
        use clap::Parser;
        #[derive(Parser)]
        struct W {
            #[command(flatten)]
            args: UninstallArgs,
        }
        let w = W::parse_from(["test", "--purge", "--yes"]);
        assert!(w.args.purge);
        assert!(w.args.yes);
    }
}
