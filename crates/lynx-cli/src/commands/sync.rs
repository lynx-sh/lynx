use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context as _, Result};
use lynx_core::error::LynxError;
use clap::{Args, Subcommand};

use lynx_config::snapshot::mutate_config_transaction;
use lynx_core::brand;

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub struct SyncArgs {
    #[command(subcommand)]
    pub command: SyncCommand,
}

#[derive(Subcommand)]
pub enum SyncCommand {
    /// Initialize git-backed config sync with a remote
    Init { remote: String },
    /// Commit any changes and push to remote
    Push,
    /// Fetch and merge from remote
    Pull,
    /// Show ahead/behind counts
    Status,
    /// Catch unknown subcommands for friendly error
    #[command(external_subcommand)]
    Other(Vec<String>),
}

pub fn run(args: SyncArgs) -> Result<()> {
    match args.command {
        SyncCommand::Init { remote } => cmd_init(&remote),
        SyncCommand::Push => cmd_push(),
        SyncCommand::Pull => cmd_pull(),
        SyncCommand::Status => cmd_status(),
        SyncCommand::Other(args) => {
            Err(LynxError::unknown_command(args.first().map(|s| s.as_str()).unwrap_or(""), "sync").into())
        }
    }
}

fn config_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(brand::CONFIG_DIR)
}

fn cmd_init(remote: &str) -> Result<()> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir).context("failed to create config dir")?;

    // Write .gitignore excluding secrets and ephemeral dirs.
    let gitignore = dir.join(".gitignore");
    if !gitignore.exists() {
        std::fs::write(
            &gitignore,
            "# Lynx sync — do not commit secrets or ephemeral data\n\
             snapshots/\n\
             benchmarks.jsonl\n\
             .update-check\n\
             *.env\n\
             *secret*\n\
             *secrets*\n\
             *credentials*\n\
             *private*\n",
        )
        .context("failed to write .gitignore")?;
    }

    // Init git repo if not already one.
    if !dir.join(".git").exists() {
        git(&dir, &["init"])?;
        git(&dir, &["add", ".gitignore"])?;
        git(&dir, &["commit", "-m", "chore: initial Lynx config sync"])?;
    }

    // Add / update remote.
    let remotes = git_output(&dir, &["remote"])?;
    if remotes.contains("origin") {
        git(&dir, &["remote", "set-url", "origin", remote])?;
    } else {
        git(&dir, &["remote", "add", "origin", remote])?;
    }

    // Save remote to config.
    mutate_config_transaction("sync-init", |cfg| {
        cfg.sync.remote = Some(remote.to_string());
        Ok(())
    })?;

    println!("sync initialized with remote: {remote}");
    Ok(())
}

fn cmd_push() -> Result<()> {
    let dir = config_dir();
    ensure_git_repo(&dir)?;

    // Stage all tracked files (respecting .gitignore).
    git(&dir, &["add", "-u"])?;
    git(&dir, &["add", "*.toml", ".gitignore"])?;

    // Commit only if there are staged changes.
    let status = git_output(&dir, &["status", "--porcelain"])?;
    if status.trim().is_empty() {
        println!("nothing to push — config up to date");
        return Ok(());
    }

    let msg = format!("chore: sync config {}", timestamp());
    git(&dir, &["commit", "-m", &msg])?;
    git(&dir, &["push", "origin", "HEAD"])?;
    println!("config pushed");
    Ok(())
}

fn cmd_pull() -> Result<()> {
    let dir = config_dir();
    ensure_git_repo(&dir)?;
    git(&dir, &["fetch", "origin"])?;
    git(&dir, &["merge", "--ff-only", "origin/HEAD"])?;
    println!("config pulled");
    Ok(())
}

fn cmd_status() -> Result<()> {
    let dir = config_dir();
    ensure_git_repo(&dir)?;

    git(&dir, &["fetch", "origin", "--quiet"])?;

    let ahead = git_output(&dir, &["rev-list", "--count", "HEAD..@{u}"])
        .unwrap_or_else(|_| "?".to_string());
    let behind = git_output(&dir, &["rev-list", "--count", "@{u}..HEAD"])
        .unwrap_or_else(|_| "?".to_string());

    println!(
        "sync status: {} ahead, {} behind",
        ahead.trim(),
        behind.trim()
    );
    Ok(())
}

fn ensure_git_repo(dir: &Path) -> Result<()> {
    if !dir.join(".git").exists() {
        return Err(LynxError::Config("config dir is not a git repo — run: lx sync init <remote>".into()).into());
    }
    Ok(())
}

fn git(dir: &Path, args: &[&str]) -> Result<()> {
    let status = Command::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .context("failed to run git")?;
    if !status.success() {
        return Err(LynxError::Shell(format!("git {} failed", args.join(" "))).into());
    }
    Ok(())
}

fn git_output(dir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .context("failed to run git")?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn timestamp() -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{ts}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_returns_numeric_string() {
        let ts = timestamp();
        assert!(ts.parse::<u64>().is_ok(), "timestamp should be numeric: {ts}");
        assert!(ts.len() >= 10, "timestamp should be at least 10 digits: {ts}");
    }

    #[test]
    fn ensure_git_repo_fails_without_git_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let result = ensure_git_repo(tmp.path());
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("not a git repo"), "unexpected error: {msg}");
    }

    #[test]
    fn ensure_git_repo_succeeds_with_git_dir() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".git")).unwrap();
        assert!(ensure_git_repo(tmp.path()).is_ok());
    }

    #[test]
    fn config_dir_contains_brand_config_dir() {
        let dir = config_dir();
        assert!(dir.to_string_lossy().contains(brand::CONFIG_DIR));
    }

    #[tokio::test]
    async fn sync_unknown_subcommand_errors() {
        let args = SyncArgs {
            command: SyncCommand::Other(vec!["oops".to_string()]),
        };
        let err = run(args).unwrap_err();
        assert!(err.to_string().contains("oops"));
    }
}
