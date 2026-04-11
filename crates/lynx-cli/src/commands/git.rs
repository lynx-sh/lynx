use anyhow::Result;
use clap::Args;
use std::process::{Command, Stdio};

#[derive(Args)]
pub struct GitStateArgs {}

/// `lx git-state` — gather git state and emit zsh that sets `_lynx_git_state`.
///
/// Called from plugins/git/shell/functions.zsh via:
/// ```zsh
/// git_refresh_state() { eval "$(lx git-state 2>/dev/null)" }
/// ```
///
/// Output when in a git repo:
/// ```
/// _lynx_git_state=(root '/path' branch 'main' dirty '1' stash '0' ahead '0' behind '0')
/// ```
///
/// Output when not in a git repo (or git unavailable):
/// ```
/// _lynx_git_state=()
/// ```
pub async fn run(_args: GitStateArgs) -> Result<()> {
    let state = gather_git_state();
    print!("{}", render_zsh(&state));
    Ok(())
}

struct GitState {
    root: Option<String>,
    branch: Option<String>,
    dirty: bool,
    stash_count: u32,
    ahead: u32,
    behind: u32,
}

/// Run a git subcommand, capture stdout. Returns `None` on non-zero exit or spawn failure.
fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .args(args)
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        None
    }
}

fn gather_git_state() -> GitState {
    let root = git(&["rev-parse", "--show-toplevel"]);
    if root.is_none() {
        return GitState {
            root: None,
            branch: None,
            dirty: false,
            stash_count: 0,
            ahead: 0,
            behind: 0,
        };
    }

    let branch = git(&["symbolic-ref", "--short", "HEAD"])
        .or_else(|| git(&["rev-parse", "--short", "HEAD"]));

    let dirty = git(&["status", "--porcelain"])
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    let stash_count = git(&["stash", "list"])
        .map(|s| s.lines().filter(|l| !l.is_empty()).count() as u32)
        .unwrap_or(0);

    let (ahead, behind) = upstream_counts();

    GitState {
        root,
        branch,
        dirty,
        stash_count,
        ahead,
        behind,
    }
}

fn upstream_counts() -> (u32, u32) {
    let upstream = match git(&["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]) {
        Some(u) if !u.is_empty() => u,
        _ => return (0, 0),
    };

    let counts = match git(&[
        "rev-list",
        "--left-right",
        "--count",
        &format!("HEAD...{upstream}"),
    ]) {
        Some(c) => c,
        None => return (0, 0),
    };

    let parts: Vec<&str> = counts.split_whitespace().collect();
    if parts.len() != 2 {
        return (0, 0);
    }
    let ahead = parts[0].parse().unwrap_or(0);
    let behind = parts[1].parse().unwrap_or(0);
    (ahead, behind)
}

/// Escape a string for use in a single-quoted zsh word.
fn zsh_escape(s: &str) -> String {
    s.replace('\'', "'\\''")
}

fn render_zsh(state: &GitState) -> String {
    if state.root.is_none() {
        return "_lynx_git_state=()\n".to_string();
    }

    let root = zsh_escape(state.root.as_deref().unwrap_or(""));
    let branch = zsh_escape(state.branch.as_deref().unwrap_or(""));
    let dirty = if state.dirty { "1" } else { "0" };

    format!(
        "_lynx_git_state=(root '{root}' branch '{branch}' dirty '{dirty}' stash '{stash}' ahead '{ahead}' behind '{behind}')\n",
        stash = state.stash_count,
        ahead = state.ahead,
        behind = state.behind,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_zsh_clears_when_not_in_repo() {
        let state = GitState {
            root: None,
            branch: None,
            dirty: false,
            stash_count: 0,
            ahead: 0,
            behind: 0,
        };
        assert_eq!(render_zsh(&state), "_lynx_git_state=()\n");
    }

    #[test]
    fn render_zsh_sets_all_fields() {
        let state = GitState {
            root: Some("/home/user/repo".into()),
            branch: Some("main".into()),
            dirty: true,
            stash_count: 2,
            ahead: 1,
            behind: 3,
        };
        let out = render_zsh(&state);
        assert!(out.contains("root '/home/user/repo'"));
        assert!(out.contains("branch 'main'"));
        assert!(out.contains("dirty '1'"));
        assert!(out.contains("stash '2'"));
        assert!(out.contains("ahead '1'"));
        assert!(out.contains("behind '3'"));
    }

    #[test]
    fn render_zsh_dirty_false_emits_zero() {
        let state = GitState {
            root: Some("/repo".into()),
            branch: Some("feat/x".into()),
            dirty: false,
            stash_count: 0,
            ahead: 0,
            behind: 0,
        };
        assert!(render_zsh(&state).contains("dirty '0'"));
    }

    #[test]
    fn zsh_escape_handles_single_quotes() {
        assert_eq!(zsh_escape("it's"), "it'\\''s");
    }

    #[test]
    fn zsh_escape_plain_string_unchanged() {
        assert_eq!(zsh_escape("main"), "main");
    }
}
