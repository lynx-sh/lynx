// Individual health checks for Lynx environment.
//
// Each function returns one Check. They never abort or short-circuit — the
// caller collects all results and presents them together so the user can fix
// multiple issues in one pass.

use crate::{Check, Status};
use lynx_config::load;
use lynx_core::diag;
use lynx_theme::loader::load as load_theme;

/// Run every check and return results in display order.
pub(crate) fn run_all() -> Vec<Check> {
    vec![
        check_zsh_version(),
        check_lx_on_path(),
        check_config_valid(),
        check_plugin_binary_deps(),
        check_shell_integration(),
        check_active_theme_valid(),
        check_diag_log(),
    ]
}

fn check_zsh_version() -> Check {
    let output = std::process::Command::new("zsh").arg("--version").output();

    match output {
        Ok(o) => {
            let ver_str = String::from_utf8_lossy(&o.stdout);
            if let Some(ver) = parse_zsh_version(&ver_str) {
                if ver >= (5, 8) {
                    Check {
                        name: "zsh >= 5.8",
                        status: Status::Pass,
                        detail: ver_str.trim().to_string(),
                        fix: None,
                    }
                } else {
                    Check {
                        name: "zsh >= 5.8",
                        status: Status::Fail,
                        detail: format!("found zsh {}.{}", ver.0, ver.1),
                        fix: Some("brew upgrade zsh".to_string()),
                    }
                }
            } else {
                Check {
                    name: "zsh >= 5.8",
                    status: Status::Warn,
                    detail: "could not parse zsh version".to_string(),
                    fix: None,
                }
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

pub(crate) fn parse_zsh_version(s: &str) -> Option<(u32, u32)> {
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
            fix: Some(
                "ln -sf $(realpath lx) ~/.local/bin/lx && export PATH=$HOME/.local/bin:$PATH"
                    .to_string(),
            ),
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
        if let Some(path) = plugin_manifest_path(plugin) {
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
        Check {
            name: "plugin binary deps",
            status: Status::Pass,
            detail: "all deps present".to_string(),
            fix: None,
        }
    } else {
        let fix_parts: Vec<String> = missing
            .iter()
            .map(|m: &String| {
                let bin = m.split('\'').nth(1).unwrap_or("unknown");
                format!("brew install {bin}")
            })
            .collect();
        Check {
            name: "plugin binary deps",
            status: Status::Warn,
            detail: missing.join("; "),
            fix: Some(fix_parts.join(" && ")),
        }
    }
}

/// Resolve plugin.toml under ~/.config/lynx/plugins/<name>.
fn plugin_manifest_path(plugin: &str) -> Option<std::path::PathBuf> {
    let path = lynx_core::paths::installed_plugins_dir()
        .join(plugin)
        .join(lynx_core::brand::PLUGIN_MANIFEST);
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

fn check_shell_integration() -> Check {
    let home = match std::env::var_os("HOME") {
        Some(h) => std::path::PathBuf::from(h),
        None => {
            return Check {
                name: "shell integration in .zshrc",
                status: Status::Warn,
                detail: "$HOME not set".to_string(),
                fix: None,
            }
        }
    };
    let zshrc = home.join(".zshrc");
    match std::fs::read_to_string(&zshrc) {
        Ok(content) => {
            if content.contains(lynx_core::brand::ZSHRC_INIT_LINE)
                || content.contains("eval \"$(lx init")
            {
                Check {
                    name: "shell integration in .zshrc",
                    status: Status::Pass,
                    detail: "Lynx source line found".to_string(),
                    fix: None,
                }
            } else {
                Check {
                    name: "shell integration in .zshrc",
                    status: Status::Fail,
                    detail: "Lynx source line not found in ~/.zshrc".to_string(),
                    fix: Some(format!(
                        "echo '{}' >> ~/.zshrc",
                        lynx_core::brand::ZSHRC_INIT_LINE
                    )),
                }
            }
        }
        Err(_) => Check {
            name: "shell integration in .zshrc",
            status: Status::Warn,
            detail: "~/.zshrc not found".to_string(),
            fix: Some(format!(
                "echo '{}' >> ~/.zshrc",
                lynx_core::brand::ZSHRC_INIT_LINE
            )),
        },
    }
}

fn check_diag_log() -> Check {
    let lines = diag::tail(20);
    let errors: Vec<&String> = lines.iter().filter(|l| l.contains("[ERROR]")).collect();
    let warns: Vec<&String> = lines.iter().filter(|l| l.contains("[WARN]")).collect();

    if errors.is_empty() && warns.is_empty() {
        Check {
            name: "diagnostic log",
            status: Status::Pass,
            detail: "no errors or warnings logged".to_string(),
            fix: None,
        }
    } else if !errors.is_empty() {
        Check {
            name: "diagnostic log",
            status: Status::Fail,
            detail: format!(
                "{} error(s), {} warning(s) in log — run `lx diag` for details",
                errors.len(),
                warns.len()
            ),
            fix: Some("lx diag".to_string()),
        }
    } else {
        Check {
            name: "diagnostic log",
            status: Status::Warn,
            detail: format!(
                "{} warning(s) in log — run `lx diag` for details",
                warns.len()
            ),
            fix: Some("lx diag".to_string()),
        }
    }
}

fn check_active_theme_valid() -> Check {
    let cfg = match load() {
        Ok(c) => c,
        Err(_) => {
            return Check {
                name: "active theme valid",
                status: Status::Warn,
                detail: "skipped — config invalid".to_string(),
                fix: None,
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use lynx_test_utils::temp_home;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn set_env_or_remove(key: &str, value: Option<&str>) {
        if let Some(v) = value {
            std::env::set_var(key, v);
        } else {
            std::env::remove_var(key);
        }
    }

    struct EnvGuard {
        saved: Vec<(String, Option<String>)>,
    }

    impl EnvGuard {
        fn new(keys: &[&str]) -> Self {
            let mut saved = Vec::new();
            for key in keys {
                saved.push(((*key).to_string(), std::env::var(key).ok()));
            }
            Self { saved }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, val) in &self.saved {
                set_env_or_remove(key, val.as_deref());
            }
        }
    }

    fn write_valid_config(home: &std::path::Path, enabled_plugins: &[&str]) {
        let cfg_dir = home.join(lynx_core::brand::CONFIG_DIR);
        std::fs::create_dir_all(&cfg_dir).expect("create config dir");
        let plugins = if enabled_plugins.is_empty() {
            "[]".to_string()
        } else {
            format!(
                "[{}]",
                enabled_plugins
                    .iter()
                    .map(|p| format!("\"{p}\""))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        std::fs::write(
            cfg_dir.join(lynx_core::brand::CONFIG_FILE),
            format!(
                "schema_version = 1\nenabled_plugins = {plugins}\nactive_theme = \"default\"\nactive_context = \"interactive\"\n"
            ),
        )
        .expect("write config");
    }

    #[test]
    fn parse_zsh_version_parses_major_minor() {
        assert_eq!(
            parse_zsh_version("zsh 5.9 (x86_64-apple-darwin)"),
            Some((5, 9))
        );
    }

    #[test]
    fn parse_zsh_version_rejects_invalid_input() {
        assert_eq!(parse_zsh_version("not-zsh"), None);
    }

    #[test]
    fn config_valid_check_passes_for_clean_config() {
        let _lock = env_lock().lock().expect("lock");
        let _guard = EnvGuard::new(&["HOME", "LYNX_DIR"]);
        let home = temp_home();
        std::env::set_var("HOME", home.path());
        std::env::remove_var("LYNX_DIR");
        write_valid_config(home.path(), &[]);

        let check = check_config_valid();
        assert_eq!(check.status, Status::Pass);
    }

    #[test]
    fn plugin_binary_deps_warns_when_binary_missing() {
        let _lock = env_lock().lock().expect("lock");
        let _guard = EnvGuard::new(&["HOME", "LYNX_DIR"]);
        let home = temp_home();
        std::env::set_var("HOME", home.path());
        std::env::remove_var("LYNX_DIR");
        write_valid_config(home.path(), &["demo"]);

        let plugin_dir = home.path().join(lynx_core::brand::CONFIG_DIR).join("plugins/demo");
        std::fs::create_dir_all(&plugin_dir).expect("create plugin dir");
        std::fs::write(
            plugin_dir.join(lynx_core::brand::PLUGIN_MANIFEST),
            r#"[plugin]
name = "demo"
version = "0.1.0"
description = "demo"
authors = ["test"]

[load]
lazy = false
hooks = []

[deps]
binaries = ["definitely-not-a-real-binary-lynx"]
plugins = []

[exports]
functions = []
aliases = []

[contexts]
disabled_in = ["agent", "minimal"]
"#,
        )
        .expect("write plugin manifest");

        let check = check_plugin_binary_deps();
        assert_eq!(check.status, Status::Warn);
        assert!(check.detail.contains("demo: requires"));
    }

    #[test]
    fn plugin_binary_deps_passes_when_no_plugins_enabled() {
        let _lock = env_lock().lock().expect("lock");
        let _guard = EnvGuard::new(&["HOME", "LYNX_DIR"]);
        let home = temp_home();
        std::env::set_var("HOME", home.path());
        std::env::remove_var("LYNX_DIR");
        write_valid_config(home.path(), &[]);

        let check = check_plugin_binary_deps();
        assert_eq!(check.status, Status::Pass);
    }

    #[test]
    fn shell_integration_warns_when_zshrc_missing() {
        let _lock = env_lock().lock().expect("lock");
        let _guard = EnvGuard::new(&["HOME", "LYNX_DIR"]);
        let home = temp_home();
        std::env::set_var("HOME", home.path());
        std::env::remove_var("LYNX_DIR");

        let check = check_shell_integration();
        assert_eq!(check.status, Status::Warn);
        assert!(check.detail.contains("~/.zshrc not found"));
    }
}
