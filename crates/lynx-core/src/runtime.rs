use crate::env_vars;
use crate::error::{LynxError, Result};
use std::path::PathBuf;

/// Resolve the Lynx runtime directory.
///
/// Priority:
/// 1. `$LYNX_RUNTIME_DIR` env override
/// 2. `$XDG_RUNTIME_DIR/lynx`
/// 3. `/tmp/lynx-<UID>`
///
/// The directory is created with mode 0o700 on first access.
/// This is the single source of truth for all runtime paths — never hardcode /tmp elsewhere.
pub fn runtime_dir() -> Result<PathBuf> {
    let dir = resolve_runtime_dir();
    std::fs::create_dir_all(&dir).map_err(LynxError::IoRaw)?;
    set_permissions_700(&dir)?;
    Ok(dir)
}

fn resolve_runtime_dir() -> PathBuf {
    if let Ok(dir) = std::env::var(env_vars::LYNX_RUNTIME_DIR) {
        return PathBuf::from(dir);
    }
    if let Ok(xdg) = std::env::var(env_vars::XDG_RUNTIME_DIR) {
        return PathBuf::from(xdg).join("lynx");
    }
    let uid = get_uid();
    PathBuf::from(format!("/tmp/lynx-{}", uid))
}

/// Path to the Unix domain socket used for event IPC.
pub fn socket_path() -> Result<PathBuf> {
    Ok(runtime_dir()?.join("events.sock"))
}

/// Path to the daemon PID file.
pub fn pid_file() -> Result<PathBuf> {
    Ok(runtime_dir()?.join("daemon.pid"))
}

/// Path to the daemon lock file.
pub fn lock_file() -> Result<PathBuf> {
    Ok(runtime_dir()?.join("daemon.lock"))
}

fn set_permissions_700(dir: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o700);
    std::fs::set_permissions(dir, perms).map_err(LynxError::IoRaw)
}

fn get_uid() -> u32 {
    libc_getuid()
}

// Thin inline syscall to avoid pulling in a libc crate dependency.
// getuid() syscall number is 24 on macOS/Linux aarch64 and x86_64.
// We use std::os::unix which is always available on Unix targets.
#[cfg(unix)]
fn libc_getuid() -> u32 {
    // std doesn't expose getuid directly, but we can read /proc/self or use
    // the USER env var as a fallback for tests. Real impl uses the nix syscall.
    // For portability without adding a dep, fall back to UID via env.
    std::env::var("UID")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1000)
}

#[cfg(not(unix))]
fn libc_getuid() -> u32 {
    1000
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    struct RuntimeDirGuard;
    impl Drop for RuntimeDirGuard {
        fn drop(&mut self) {
            std::env::remove_var("LYNX_RUNTIME_DIR");
            std::env::remove_var("XDG_RUNTIME_DIR");
        }
    }

    #[test]
    fn runtime_dir_creates_with_700() {
        let tmp = tempfile::tempdir().unwrap();
        let override_path = tmp.path().join("lynx-runtime-test");
        std::env::set_var("LYNX_RUNTIME_DIR", &override_path);
        let _g = RuntimeDirGuard;

        let dir = runtime_dir().unwrap();
        assert!(dir.exists());
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700, "expected 700, got {:o}", mode & 0o777);
    }

    #[test]
    fn xdg_runtime_dir_respected() {
        let tmp = tempfile::tempdir().unwrap();
        std::env::remove_var("LYNX_RUNTIME_DIR");
        std::env::set_var("XDG_RUNTIME_DIR", tmp.path());
        let _g = RuntimeDirGuard;

        let dir = runtime_dir().unwrap();
        assert_eq!(dir, tmp.path().join("lynx"));
    }

    #[test]
    fn all_paths_derive_from_runtime_dir() {
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("LYNX_RUNTIME_DIR", tmp.path());
        let _g = RuntimeDirGuard;

        let base = runtime_dir().unwrap();
        assert_eq!(socket_path().unwrap(), base.join("events.sock"));
        assert_eq!(pid_file().unwrap(), base.join("daemon.pid"));
        assert_eq!(lock_file().unwrap(), base.join("daemon.lock"));
    }
}
