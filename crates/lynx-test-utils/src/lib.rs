use lynx_config::schema::LynxConfig;
use lynx_core::types::Context;
use std::path::PathBuf;
use tempfile::TempDir;

/// Creates an isolated temp directory suitable for use as a fake `$HOME` in tests.
/// The caller must keep the returned `TempDir` alive for the duration of the test.
pub fn temp_home() -> TempDir {
    tempfile::tempdir().expect("failed to create temp home")
}

/// Returns the path to the named fixture plugin directory under `plugins/`.
///
/// Caller is responsible for ensuring the fixture exists in the repo.
pub fn fixture_plugin(name: &str) -> PathBuf {
    let repo_root = repo_root();
    repo_root.join("plugins").join(name)
}

/// Returns a [`LynxConfig`] populated with stable test defaults.
pub fn fixture_config() -> LynxConfig {
    LynxConfig {
        active_theme: "test-theme".into(),
        active_context: Context::Interactive,
        enabled_plugins: vec!["git".into()],
        ..Default::default()
    }
}

/// Syntax-check a zsh code snippet using `zsh -n`.
///
/// # Panics
/// Panics with the zsh stderr output if the syntax check fails.
pub fn assert_valid_zsh(code: &str) {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = Command::new("zsh")
        .args(["-n", "/dev/stdin"])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn zsh — is zsh installed?");

    child
        .stdin
        .take()
        .unwrap()
        .write_all(code.as_bytes())
        .unwrap();

    let output = child.wait_with_output().unwrap();
    if !output.status.success() {
        panic!(
            "zsh syntax check failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn repo_root() -> PathBuf {
    // Walk up from CARGO_MANIFEST_DIR until we find Cargo.toml with [workspace]
    let start = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut dir = start.as_path();
    loop {
        let candidate = dir.join("Cargo.toml");
        if candidate.exists() {
            if let Ok(content) = std::fs::read_to_string(&candidate) {
                if content.contains("[workspace]") {
                    return dir.to_path_buf();
                }
            }
        }
        dir = dir.parent().expect("reached filesystem root without finding workspace");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_home_creates_and_cleans_up() {
        let home = temp_home();
        let path = home.path().to_path_buf();
        assert!(path.exists());
        drop(home);
        assert!(!path.exists());
    }

    #[test]
    fn fixture_plugin_returns_valid_path() {
        let p = fixture_plugin("git");
        assert!(p.exists(), "plugins/git fixture not found at {:?}", p);
        assert!(p.join("plugin.toml").exists());
    }

    #[test]
    fn fixture_config_is_valid() {
        let cfg = fixture_config();
        assert_eq!(cfg.active_theme, "test-theme");
        assert!(cfg.enabled_plugins.contains(&"git".to_string()));
    }

    #[test]
    fn assert_valid_zsh_passes_good_code() {
        assert_valid_zsh("echo hello");
    }

    #[test]
    #[should_panic(expected = "zsh syntax check failed")]
    fn assert_valid_zsh_catches_bad_code() {
        assert_valid_zsh("if then done fi (((");
    }
}
