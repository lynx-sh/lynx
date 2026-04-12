use anyhow::Result;
use clap::Args;
use std::process::{Command, Stdio};

#[derive(Args)]
pub struct GitStateArgs {}

/// `lx git-state` — gather git state and emit zsh that sets `_lynx_git_state`
/// and exports `LYNX_CACHE_GIT_STATE` as JSON for the prompt cache.
///
/// Called from plugins/git/shell/functions.zsh via:
/// ```zsh
/// git_refresh_state() { eval "$(lx git-state 2>/dev/null)" }
/// ```
///
/// Output when in a git repo:
/// ```
/// _lynx_git_state=(root '/path' branch 'main' dirty '1' staged '1' modified '0' untracked '0' stash '0' ahead '0' behind '0')
/// export LYNX_CACHE_GIT_STATE='{"branch":"main","dirty":true,"staged":true,"modified":false,"untracked":false,"stash":0,"ahead":0,"behind":0}'
/// ```
///
/// Output when not in a git repo (or git unavailable):
/// ```
/// _lynx_git_state=()
/// export LYNX_CACHE_GIT_STATE=''
/// ```
pub async fn run(_args: GitStateArgs) -> Result<()> {
    let state = gather_git_state();
    print!("{}", render_zsh(&state));
    Ok(())
}

pub(crate) struct GitState {
    root: Option<String>,
    branch: Option<String>,
    dirty: bool,
    staged: bool,
    modified: bool,
    untracked: bool,
    stash_count: u32,
    ahead: u32,
    behind: u32,
    /// Active repo action (merge, rebase, cherry-pick, bisect) or None when idle.
    action: Option<String>,
    /// Short commit SHA (from rev-parse --short HEAD).
    sha: Option<String>,
    /// Unix timestamp of the last commit (from git log --format=%at -1).
    commit_ts: Option<u64>,
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

pub(crate) fn gather_git_state() -> GitState {
    let root = git(&["rev-parse", "--show-toplevel"]);
    if root.is_none() {
        return GitState {
            root: None,
            branch: None,
            dirty: false,
            staged: false,
            modified: false,
            untracked: false,
            stash_count: 0,
            ahead: 0,
            behind: 0,
            action: None,
            sha: None,
            commit_ts: None,
        };
    }

    let branch = git(&["symbolic-ref", "--short", "HEAD"])
        .or_else(|| git(&["rev-parse", "--short", "HEAD"]));

    let (dirty, staged, modified, untracked) =
        parse_porcelain(git(&["status", "--porcelain"]).as_deref().unwrap_or(""));

    let stash_count = git(&["stash", "list"])
        .map(|s| s.lines().filter(|l| !l.is_empty()).count() as u32)
        .unwrap_or(0);

    let (ahead, behind) = upstream_counts();
    let action = detect_action(root.as_deref().unwrap_or(""));
    let sha = git(&["rev-parse", "HEAD"]);
    let commit_ts = git(&["log", "--format=%at", "-1"])
        .and_then(|s| s.parse::<u64>().ok());

    GitState {
        root,
        branch,
        dirty,
        staged,
        modified,
        untracked,
        stash_count,
        ahead,
        behind,
        action,
        sha,
        commit_ts,
    }
}

/// Detect the active git operation via filesystem checks on the repo root.
/// No git subprocess — reads only well-known marker files/directories.
pub(crate) fn detect_action(root: &str) -> Option<String> {
    use std::path::Path;
    let git_dir = Path::new(root).join(".git");
    if git_dir.join("MERGE_HEAD").exists() {
        return Some("merge".to_string());
    }
    if git_dir.join("rebase-merge").is_dir() || git_dir.join("rebase-apply").is_dir() {
        return Some("rebase".to_string());
    }
    if git_dir.join("CHERRY_PICK_HEAD").exists() {
        return Some("cherry-pick".to_string());
    }
    if git_dir.join("BISECT_LOG").exists() {
        return Some("bisect".to_string());
    }
    None
}

/// Parse `git status --porcelain` output into (dirty, staged, modified, untracked).
///
/// Each line is two characters: XY where X = index status, Y = worktree status.
/// - staged:    X is not ' ' or '?'
/// - modified:  Y is 'M', 'D', or 'T'
/// - untracked: line starts with '??'
fn parse_porcelain(porcelain: &str) -> (bool, bool, bool, bool) {
    let mut staged = false;
    let mut modified = false;
    let mut untracked = false;

    for line in porcelain.lines() {
        if line.len() < 2 {
            continue;
        }
        let x = line.chars().next().unwrap_or(' ');
        let y = line.chars().nth(1).unwrap_or(' ');

        if x == '?' && y == '?' {
            untracked = true;
        } else {
            if x != ' ' {
                staged = true;
            }
            if matches!(y, 'M' | 'D' | 'T') {
                modified = true;
            }
        }
    }

    let dirty = staged || modified || untracked;
    (dirty, staged, modified, untracked)
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

pub(crate) fn render_zsh(state: &GitState) -> String {
    if state.root.is_none() {
        return "_lynx_git_state=()\nexport LYNX_CACHE_GIT_STATE=''\n".to_string();
    }

    let root = zsh_escape(state.root.as_deref().unwrap_or(""));
    let branch = zsh_escape(state.branch.as_deref().unwrap_or(""));
    let dirty = if state.dirty { "1" } else { "0" };
    let staged = if state.staged { "1" } else { "0" };
    let modified = if state.modified { "1" } else { "0" };
    let untracked = if state.untracked { "1" } else { "0" };

    // JSON for LYNX_CACHE_GIT_STATE — read by lx prompt render into the segment cache.
    // branch is JSON-escaped to handle unusual names safely.
    let branch_raw = state.branch.as_deref().unwrap_or("");
    let branch_json = branch_raw.replace('\\', "\\\\").replace('"', "\\\"");

    let action_zsh = state.action.as_deref().unwrap_or("");
    let action_json = match &state.action {
        Some(a) => format!(r#""{}""#, a.replace('"', "\\\"")),
        None => "null".to_string(),
    };

    let sha_zsh = state.sha.as_deref().unwrap_or("");
    let sha_json = match &state.sha {
        Some(s) => format!(r#""{}""#, s.replace('"', "\\\"")),
        None => "null".to_string(),
    };

    let commit_ts_zsh = state.commit_ts.map(|t| t.to_string()).unwrap_or_default();
    let commit_ts_json = match state.commit_ts {
        Some(t) => t.to_string(),
        None => "null".to_string(),
    };

    let json = format!(
        r#"{{"branch":"{branch_json}","dirty":{dirty_b},"staged":{staged_b},"modified":{modified_b},"untracked":{untracked_b},"stash":{stash},"ahead":{ahead},"behind":{behind},"action":{action_json},"sha":{sha_json},"commit_ts":{commit_ts_json}}}"#,
        dirty_b = state.dirty,
        staged_b = state.staged,
        modified_b = state.modified,
        untracked_b = state.untracked,
        stash = state.stash_count,
        ahead = state.ahead,
        behind = state.behind,
    );

    format!(
        "_lynx_git_state=(root '{root}' branch '{branch}' dirty '{dirty}' staged '{staged}' modified '{modified}' untracked '{untracked}' stash '{stash}' ahead '{ahead}' behind '{behind}' action '{action_zsh}' sha '{sha_zsh}' commit_ts '{commit_ts_zsh}')\nexport LYNX_CACHE_GIT_STATE='{json}'\n",
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
            staged: false,
            modified: false,
            untracked: false,
            stash_count: 0,
            ahead: 0,
            behind: 0,
            action: None,
            sha: None,
            commit_ts: None,
        };
        let out = render_zsh(&state);
        assert!(out.contains("_lynx_git_state=()"));
        assert!(out.contains("export LYNX_CACHE_GIT_STATE=''"));
    }

    #[test]
    fn render_zsh_sets_all_fields() {
        let state = GitState {
            root: Some("/home/user/repo".into()),
            branch: Some("main".into()),
            dirty: true,
            staged: true,
            modified: false,
            untracked: false,
            stash_count: 2,
            ahead: 1,
            behind: 3,
            action: None,
            sha: Some("abc1234def5678".into()),
            commit_ts: Some(1700000000),
        };
        let out = render_zsh(&state);
        assert!(out.contains("root '/home/user/repo'"));
        assert!(out.contains("branch 'main'"));
        assert!(out.contains("dirty '1'"));
        assert!(out.contains("staged '1'"));
        assert!(out.contains("modified '0'"));
        assert!(out.contains("untracked '0'"));
        assert!(out.contains("stash '2'"));
        assert!(out.contains("ahead '1'"));
        assert!(out.contains("behind '3'"));
    }

    #[test]
    fn render_zsh_exports_json_cache() {
        let state = GitState {
            root: Some("/repo".into()),
            branch: Some("feat/x".into()),
            dirty: true,
            staged: true,
            modified: true,
            untracked: false,
            stash_count: 0,
            ahead: 0,
            behind: 0,
            action: None,
            sha: None,
            commit_ts: None,
        };
        let out = render_zsh(&state);
        assert!(out.contains("export LYNX_CACHE_GIT_STATE='"));
        assert!(out.contains(r#""branch":"feat/x""#));
        assert!(out.contains(r#""staged":true"#));
        assert!(out.contains(r#""modified":true"#));
        assert!(out.contains(r#""untracked":false"#));
    }

    #[test]
    fn render_zsh_dirty_false_emits_zero() {
        let state = GitState {
            root: Some("/repo".into()),
            branch: Some("feat/x".into()),
            dirty: false,
            staged: false,
            modified: false,
            untracked: false,
            stash_count: 0,
            ahead: 0,
            behind: 0,
            action: None,
            sha: None,
            commit_ts: None,
        };
        let out = render_zsh(&state);
        assert!(out.contains("dirty '0'"));
        assert!(out.contains(r#""dirty":false"#));
    }

    #[test]
    fn parse_porcelain_clean() {
        let (dirty, staged, modified, untracked) = parse_porcelain("");
        assert!(!dirty && !staged && !modified && !untracked);
    }

    #[test]
    fn parse_porcelain_staged_only() {
        let (dirty, staged, modified, untracked) = parse_porcelain("A  new_file.rs\n");
        assert!(dirty && staged && !modified && !untracked);
    }

    #[test]
    fn parse_porcelain_modified_only() {
        let (dirty, staged, modified, untracked) = parse_porcelain(" M src/lib.rs\n");
        assert!(dirty && !staged && modified && !untracked);
    }

    #[test]
    fn parse_porcelain_untracked_only() {
        let (dirty, staged, modified, untracked) = parse_porcelain("?? scratch.rs\n");
        assert!(dirty && !staged && !modified && untracked);
    }

    #[test]
    fn parse_porcelain_all_three() {
        let input = "M  staged.rs\n M modified.rs\n?? new.rs\n";
        let (dirty, staged, modified, untracked) = parse_porcelain(input);
        assert!(dirty && staged && modified && untracked);
    }

    #[test]
    fn zsh_escape_handles_single_quotes() {
        assert_eq!(zsh_escape("it's"), "it'\\''s");
    }

    #[test]
    fn zsh_escape_plain_string_unchanged() {
        assert_eq!(zsh_escape("main"), "main");
    }

    #[test]
    fn render_zsh_includes_action_null() {
        let state = GitState {
            root: Some("/repo".into()),
            branch: Some("main".into()),
            dirty: false,
            staged: false,
            modified: false,
            untracked: false,
            stash_count: 0,
            ahead: 0,
            behind: 0,
            action: None,
            sha: None,
            commit_ts: None,
        };
        let out = render_zsh(&state);
        assert!(out.contains(r#""action":null"#), "action field missing: {out}");
        assert!(out.contains("action ''"), "action zsh field missing: {out}");
    }

    #[test]
    fn render_zsh_includes_action_merge() {
        let state = GitState {
            root: Some("/repo".into()),
            branch: Some("main".into()),
            dirty: false,
            staged: false,
            modified: false,
            untracked: false,
            stash_count: 0,
            ahead: 0,
            behind: 0,
            action: Some("merge".into()),
            sha: None,
            commit_ts: None,
        };
        let out = render_zsh(&state);
        assert!(out.contains(r#""action":"merge""#), "action json missing: {out}");
        assert!(out.contains("action 'merge'"), "action zsh missing: {out}");
    }

    #[test]
    fn detect_action_returns_none_without_markers() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        let result = detect_action(dir.path().to_str().unwrap());
        assert!(result.is_none());
    }

    #[test]
    fn detect_action_merge_head() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path().join(".git");
        std::fs::create_dir_all(&git_dir).unwrap();
        std::fs::write(git_dir.join("MERGE_HEAD"), "abc123").unwrap();
        assert_eq!(detect_action(dir.path().to_str().unwrap()), Some("merge".into()));
    }

    #[test]
    fn detect_action_rebase_merge_dir() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path().join(".git");
        std::fs::create_dir_all(git_dir.join("rebase-merge")).unwrap();
        assert_eq!(detect_action(dir.path().to_str().unwrap()), Some("rebase".into()));
    }

    #[test]
    fn detect_action_cherry_pick() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path().join(".git");
        std::fs::create_dir_all(&git_dir).unwrap();
        std::fs::write(git_dir.join("CHERRY_PICK_HEAD"), "abc123").unwrap();
        assert_eq!(detect_action(dir.path().to_str().unwrap()), Some("cherry-pick".into()));
    }

    #[test]
    fn detect_action_bisect() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path().join(".git");
        std::fs::create_dir_all(&git_dir).unwrap();
        std::fs::write(git_dir.join("BISECT_LOG"), "").unwrap();
        assert_eq!(detect_action(dir.path().to_str().unwrap()), Some("bisect".into()));
    }
}
