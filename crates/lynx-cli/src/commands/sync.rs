use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Context as _, Result};
use clap::{Args, Subcommand};

use lynx_config::{load, save};
use lynx_core::brand;

#[derive(Args)]
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
}

pub async fn run(args: SyncArgs) -> Result<()> {
    match args.command {
        SyncCommand::Init { remote } => cmd_init(&remote),
        SyncCommand::Push => cmd_push(),
        SyncCommand::Pull => cmd_pull(),
        SyncCommand::Status => cmd_status(),
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
    let mut cfg = load()?;
    cfg.sync.remote = Some(remote.to_string());
    save(&cfg)?;

    println!("sync initialized with remote: {remote}");
    Ok(())
}

fn cmd_push() -> Result<()> {
    let dir = config_dir();
    ensure_git_repo(&dir)?;

    // Stage all tracked files (respecting .gitignore).
    git(&dir, &["add", "-u"])?;
    // Explicitly include profiles/ so new profile files are synced.
    git(&dir, &["add", "*.toml", "profiles/", ".gitignore"])?;

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

    println!("sync status: {} ahead, {} behind", ahead.trim(), behind.trim());
    Ok(())
}

fn ensure_git_repo(dir: &PathBuf) -> Result<()> {
    if !dir.join(".git").exists() {
        bail!("config dir is not a git repo — run: lx sync init <remote>");
    }
    Ok(())
}

fn git(dir: &PathBuf, args: &[&str]) -> Result<()> {
    let status = Command::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .context("failed to run git")?;
    if !status.success() {
        bail!("git {} failed", args.join(" "));
    }
    Ok(())
}

fn git_output(dir: &PathBuf, args: &[&str]) -> Result<String> {
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
