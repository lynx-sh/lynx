//! Single source of truth for all Lynx filesystem paths.
//!
//! All paths are derived from `$HOME` (or `$LYNX_DIR` where applicable).
//! Never construct Lynx paths inline — always call these functions.
//!
//! Path layout:
//! ```text
//! ~/.config/lynx/
//!   config.toml       ← config_file()
//!   tasks.toml        ← tasks_file()
//!   snapshots/        ← snapshots_dir()
//!   logs/             ← logs_dir()
//!     tasks/          ← task_logs_dir()
//!     events.jsonl    ← events_log_file()
//!   themes/           ← themes_dir()
//!   plugins/          ← installed_plugins_dir()
//!   taps.toml         ← taps_config_path()
//!   registry/         ← registry_cache_dir()
//!   shell/
//!     init.zsh        ← shell_init_file()
//! ~/.local/bin/
//!   lx                ← cli_bin()
//! ```

use crate::brand;
use crate::env_vars;
use std::path::PathBuf;

/// Resolve `$HOME` — base for all config/data paths.
///
/// Use this instead of reading `$HOME` directly so all HOME derivation
/// goes through a single place. For Lynx config paths, prefer the
/// specific helpers (`lynx_dir()`, `config_file()`, etc.) over calling
/// `home()` directly.
///
/// # Panics
///
/// Panics if `$HOME` is unset or empty. HOME is a fundamental requirement
/// for Lynx to function — there is no sensible fallback.
pub fn home() -> PathBuf {
    // Unix: $HOME is canonical.
    // Windows: fall back to $USERPROFILE, then $HOMEDRIVE+$HOMEPATH.
    let val = std::env::var_os(env_vars::HOME)
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var_os("USERPROFILE").filter(|v| !v.is_empty()))
        .or_else(|| {
            let drive = std::env::var_os("HOMEDRIVE")?;
            let path = std::env::var_os("HOMEPATH")?;
            let mut p = drive;
            p.push(path);
            Some(p)
        });

    match val {
        Some(v) => PathBuf::from(v),
        None => panic!(
            "lynx: HOME environment variable is not set and home directory could not be determined"
        ),
    }
}

/// Resolve the Lynx config/install directory.
///
/// Respects `$LYNX_DIR` override; defaults to `~/.config/lynx`.
pub fn lynx_dir() -> PathBuf {
    if let Ok(dir) = std::env::var(env_vars::LYNX_DIR) {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    home().join(brand::CONFIG_DIR)
}

/// `~/.config/lynx/config.toml`
pub fn config_file() -> PathBuf {
    lynx_dir().join(brand::CONFIG_FILE)
}

/// `~/.config/lynx/tasks.toml`
pub fn tasks_file() -> PathBuf {
    lynx_dir().join(brand::TASKS_FILE)
}

/// `~/.config/lynx/snapshots/`
pub fn snapshots_dir() -> PathBuf {
    lynx_dir().join("snapshots")
}

/// `~/.config/lynx/logs/`
pub fn logs_dir() -> PathBuf {
    lynx_dir().join("logs")
}

/// `~/.config/lynx/logs/tasks/`
pub fn task_logs_dir() -> PathBuf {
    logs_dir().join("tasks")
}

/// `~/.config/lynx/logs/events.jsonl`
pub fn events_log_file() -> PathBuf {
    logs_dir().join("events.jsonl")
}

/// `~/.config/lynx/themes/`
pub fn themes_dir() -> PathBuf {
    lynx_dir().join("themes")
}

/// `~/.config/lynx/workflows/`
pub fn workflows_dir() -> PathBuf {
    lynx_dir().join("workflows")
}

/// `~/.config/lynx/jobs/`
pub fn jobs_dir() -> PathBuf {
    lynx_dir().join("jobs")
}

/// `~/.config/lynx/plugins/` — installed plugins directory.
pub fn installed_plugins_dir() -> PathBuf {
    lynx_dir().join("plugins")
}

/// `~/.config/lynx/taps.toml` — tap registry configuration.
pub fn taps_config_path() -> PathBuf {
    lynx_dir().join(brand::TAPS_FILE)
}

/// `~/.config/lynx/registry/` — cached registry indexes.
pub fn registry_cache_dir() -> PathBuf {
    lynx_dir().join("registry")
}

/// `~/.config/lynx/lynx.lock` — installed plugin lock file.
pub fn lynx_lock_path() -> PathBuf {
    lynx_dir().join("lynx.lock")
}

/// `~/.config/lynx/benchmarks.jsonl` — benchmark results log.
pub fn benchmarks_log_file() -> PathBuf {
    lynx_dir().join("benchmarks.jsonl")
}

/// `~/.config/lynx/.update-check` — cached update version check.
pub fn update_check_file() -> PathBuf {
    lynx_dir().join(".update-check")
}

/// `~/.config/lynx/shell/init.zsh` — the shell init file sourced from `.zshrc`.
pub fn shell_init_file() -> PathBuf {
    lynx_dir().join("shell").join("init.zsh")
}

/// `~/.local/bin/lx`
pub fn cli_bin() -> PathBuf {
    home().join(".local").join("bin").join(brand::CLI)
}

/// `~/.local/bin/`
pub fn bin_dir() -> PathBuf {
    home().join(".local").join("bin")
}

/// Search `$PATH` for a binary named `name`. Returns the first match, or `None`.
///
/// This is the single canonical binary-lookup used across all crates.
/// Never write inline PATH-walking logic — call this instead.
pub fn find_binary(name: &str) -> Option<PathBuf> {
    std::env::var_os(crate::env_vars::PATH).and_then(|path| {
        std::env::split_paths(&path).find_map(|dir| {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
            // Windows: binaries have a .exe suffix that callers should not need to supply.
            #[cfg(windows)]
            {
                let exe = dir.join(format!("{name}.exe"));
                if exe.is_file() {
                    return Some(exe);
                }
            }
            None
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct HomeGuard(Option<std::ffi::OsString>);
    impl HomeGuard {
        fn set(val: &str) -> Self {
            let old = std::env::var_os(env_vars::HOME);
            std::env::set_var(env_vars::HOME, val);
            HomeGuard(old)
        }
    }
    impl Drop for HomeGuard {
        fn drop(&mut self) {
            match &self.0 {
                Some(v) => std::env::set_var(env_vars::HOME, v),
                None => std::env::remove_var(env_vars::HOME),
            }
        }
    }

    struct LynxDirGuard(Option<String>);
    impl LynxDirGuard {
        fn unset() -> Self {
            let old = std::env::var(env_vars::LYNX_DIR).ok();
            std::env::remove_var(env_vars::LYNX_DIR);
            LynxDirGuard(old)
        }
    }
    impl Drop for LynxDirGuard {
        fn drop(&mut self) {
            match &self.0 {
                Some(v) => std::env::set_var(env_vars::LYNX_DIR, v),
                None => std::env::remove_var(env_vars::LYNX_DIR),
            }
        }
    }

    #[test]
    fn all_paths_derive_from_lynx_dir() {
        let _h = HomeGuard::set("/home/testuser");
        let _l = LynxDirGuard::unset();
        let base = lynx_dir();
        assert_eq!(base, PathBuf::from("/home/testuser/.config/lynx"));
        assert_eq!(config_file(), base.join("config.toml"));
        assert_eq!(tasks_file(), base.join("tasks.toml"));
        assert_eq!(snapshots_dir(), base.join("snapshots"));
        assert_eq!(logs_dir(), base.join("logs"));
        assert_eq!(task_logs_dir(), base.join("logs").join("tasks"));
        assert_eq!(events_log_file(), base.join("logs").join("events.jsonl"));
        assert_eq!(themes_dir(), base.join("themes"));
        assert_eq!(installed_plugins_dir(), base.join("plugins"));
    }

    #[test]
    fn lynx_dir_override_respected() {
        let _h = HomeGuard::set("/home/testuser");
        std::env::set_var(env_vars::LYNX_DIR, "/custom/lynx");
        let _l = LynxDirGuard(Some("/custom/lynx".to_string()));
        assert_eq!(lynx_dir(), PathBuf::from("/custom/lynx"));
        assert_eq!(config_file(), PathBuf::from("/custom/lynx/config.toml"));
    }

    #[test]
    fn cli_bin_uses_home() {
        let _h = HomeGuard::set("/home/testuser");
        assert_eq!(cli_bin(), PathBuf::from("/home/testuser/.local/bin/lx"));
    }

    // On Windows, home() falls back to USERPROFILE / HOMEDRIVE+HOMEPATH which are
    // always set by the OS — simulating a fully-unset home is not meaningful there.
    #[cfg(unix)]
    #[test]
    #[should_panic(expected = "HOME environment variable is not set")]
    fn home_panics_when_unset() {
        // HomeGuard with empty string simulates missing HOME
        let _h = HomeGuard::set("");
        home();
    }
}
