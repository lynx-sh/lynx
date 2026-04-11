use anyhow::Result;
use clap::Args;

use lynx_config::load;
use lynx_theme::loader::load as load_theme;

#[derive(Args)]
pub struct DoctorArgs {
    /// Output results as JSON (for scripting)
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug)]
struct Check {
    name: &'static str,
    status: Status,
    detail: String,
    fix: Option<String>,
}

#[derive(Debug, PartialEq)]
enum Status {
    Pass,
    Warn,
    Fail,
}

impl Status {
    fn symbol(&self) -> &'static str {
        match self {
            Status::Pass => "✓",
            Status::Warn => "⚠",
            Status::Fail => "✗",
        }
    }
    fn label(&self) -> &'static str {
        match self {
            Status::Pass => "pass",
            Status::Warn => "warn",
            Status::Fail => "fail",
        }
    }
}

pub async fn run(args: DoctorArgs) -> Result<()> {
    let checks = run_checks();

    if args.json {
        print_json(&checks);
    } else {
        print_human(&checks);
    }

    Ok(())
}

fn run_checks() -> Vec<Check> {
    // Run all checks — never abort early.
    vec![
        check_zsh_version(),
        check_lx_on_path(),
        check_config_valid(),
        check_plugin_binary_deps(),
        check_shell_integration(),
        check_active_theme_valid(),
    ]
}

fn check_zsh_version() -> Check {
    let output = std::process::Command::new("zsh")
        .arg("--version")
        .output();

    match output {
        Ok(o) => {
            let ver_str = String::from_utf8_lossy(&o.stdout);
            // "zsh 5.9 (x86_64-apple-darwin...)"
            if let Some(ver) = parse_zsh_version(&ver_str) {
                if ver >= (5, 8) {
                    Check { name: "zsh >= 5.8", status: Status::Pass, detail: ver_str.trim().to_string(), fix: None }
                } else {
                    Check {
                        name: "zsh >= 5.8",
                        status: Status::Fail,
                        detail: format!("found zsh {}.{}", ver.0, ver.1),
                        fix: Some("brew upgrade zsh".to_string()),
                    }
                }
            } else {
                Check { name: "zsh >= 5.8", status: Status::Warn, detail: "could not parse zsh version".to_string(), fix: None }
            }
        }
        Err(e) => Check {
            name: "zsh >= 5.8",
            status: Status::Fail,
            detail: format!("zsh not found: {e}"),
            fix: Some("brew install zsh".to_string()),
        },
    }
}

fn parse_zsh_version(s: &str) -> Option<(u32, u32)> {
    let s = s.trim().strip_prefix("zsh ")?.split_whitespace().next()?;
    let mut parts = s.split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: u32 = parts.next()?.parse().ok()?;
    Some((major, minor))
}

fn check_lx_on_path() -> Check {
    match which::which("lx") {
        Ok(path) => Check {
            name: "lx binary on PATH",
            status: Status::Pass,
            detail: path.display().to_string(),
            fix: None,
        },
        Err(_) => Check {
            name: "lx binary on PATH",
            status: Status::Fail,
            detail: "lx not found in PATH".to_string(),
            fix: Some("ln -sf $(realpath lx) ~/.local/bin/lx && export PATH=$HOME/.local/bin:$PATH".to_string()),
        },
    }
}

fn check_config_valid() -> Check {
    match load() {
        Ok(_) => Check {
            name: "config.toml valid",
            status: Status::Pass,
            detail: "config loaded OK".to_string(),
            fix: None,
        },
        Err(e) => Check {
            name: "config.toml valid",
            status: Status::Fail,
            detail: e.to_string(),
            fix: Some("lx config edit".to_string()),
        },
    }
}

fn check_plugin_binary_deps() -> Check {
    let cfg = match load() {
        Ok(c) => c,
        Err(_) => {
            return Check {
                name: "plugin binary deps",
                status: Status::Warn,
                detail: "skipped — config invalid".to_string(),
                fix: None,
            };
        }
    };

    let mut missing: Vec<String> = Vec::new();
    for plugin in &cfg.enabled_plugins {
        let manifest_path = dirs_manifest(plugin);
        if let Some(path) = manifest_path {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(manifest) = toml::from_str::<toml::Value>(&content) {
                    if let Some(bins) = manifest
                        .get("deps")
                        .and_then(|d| d.get("binaries"))
                        .and_then(|b| b.as_array())
                    {
                        for bin in bins {
                            if let Some(name) = bin.as_str() {
                                if which::which(name).is_err() {
                                    missing.push(format!("{plugin}: requires '{name}'"));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if missing.is_empty() {
        Check { name: "plugin binary deps", status: Status::Pass, detail: "all deps present".to_string(), fix: None }
    } else {
        let fix_parts: Vec<String> = missing.iter().map(|m: &String| {
            let bin = m.split('\'').nth(1).unwrap_or("unknown");
            format!("brew install {bin}")
        }).collect();
        Check {
            name: "plugin binary deps",
            status: Status::Warn,
            detail: missing.join("; "),
            fix: Some(fix_parts.join(" && ")),
        }
    }
}

fn dirs_manifest(plugin: &str) -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from)?;
    let path = home.join(".config/lynx/plugins").join(plugin).join("plugin.toml");
    if path.exists() { Some(path) } else { None }
}

fn check_shell_integration() -> Check {
    let home = match std::env::var_os("HOME") {
        Some(h) => std::path::PathBuf::from(h),
        None => return Check {
            name: "shell integration in .zshrc",
            status: Status::Warn,
            detail: "$HOME not set".to_string(),
            fix: None,
        },
    };
    let zshrc = home.join(".zshrc");
    match std::fs::read_to_string(&zshrc) {
        Ok(content) => {
            if content.contains("eval \"$(lx init") || content.contains("eval \"$(lx") {
                Check { name: "shell integration in .zshrc", status: Status::Pass, detail: "LYNX_SOURCE_LINE found".to_string(), fix: None }
            } else {
                Check {
                    name: "shell integration in .zshrc",
                    status: Status::Fail,
                    detail: "lx init line not found in ~/.zshrc".to_string(),
                    fix: Some(r#"echo 'eval "$(lx init)"' >> ~/.zshrc"#.to_string()),
                }
            }
        }
        Err(_) => Check {
            name: "shell integration in .zshrc",
            status: Status::Warn,
            detail: "~/.zshrc not found".to_string(),
            fix: Some(r#"echo 'eval "$(lx init)"' >> ~/.zshrc"#.to_string()),
        },
    }
}

fn check_active_theme_valid() -> Check {
    let cfg = match load() {
        Ok(c) => c,
        Err(_) => return Check {
            name: "active theme valid",
            status: Status::Warn,
            detail: "skipped — config invalid".to_string(),
            fix: None,
        },
    };

    match load_theme(&cfg.active_theme) {
        Ok(_) => Check {
            name: "active theme valid",
            status: Status::Pass,
            detail: format!("theme '{}' loaded OK", cfg.active_theme),
            fix: None,
        },
        Err(e) => Check {
            name: "active theme valid",
            status: Status::Fail,
            detail: e.to_string(),
            fix: Some("lx theme set default".to_string()),
        },
    }
}

fn print_human(checks: &[Check]) {
    let mut any_fail = false;
    for c in checks {
        println!("  {} {}  {}", c.status.symbol(), c.name, c.detail);
        if let Some(fix) = &c.fix {
            println!("    Fix: {fix}");
        }
        if c.status == Status::Fail {
            any_fail = true;
        }
    }
    println!();
    if any_fail {
        println!("Issues found. Run the Fix commands above to resolve them.");
    } else {
        println!("All checks passed.");
    }
}

fn print_json(checks: &[Check]) {
    let items: Vec<serde_json::Value> = checks.iter().map(|c| {
        let mut obj = serde_json::json!({
            "name": c.name,
            "status": c.status.label(),
            "detail": c.detail,
        });
        if let Some(fix) = &c.fix {
            obj["fix"] = serde_json::Value::String(fix.clone());
        }
        obj
    }).collect();
    println!("{}", serde_json::to_string_pretty(&items).unwrap_or_default());
}

