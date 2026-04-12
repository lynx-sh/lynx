use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context as _, Result};
use clap::{Args, Subcommand};

use lynx_core::paths::{lynx_dir, themes_dir};

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub struct DevArgs {
    #[command(subcommand)]
    pub command: DevCommand,
}

#[derive(Subcommand)]
pub enum DevCommand {
    /// Sync source-tree assets (themes, shell, plugins) to the installed LYNX_DIR.
    /// Run after editing any asset file in the repo to see changes in your live shell.
    Sync {
        /// Path to the Lynx source tree root (default: current directory)
        #[arg(long, default_value = ".")]
        source: String,
    },
}

pub async fn run(args: DevArgs) -> Result<()> {
    match args.command {
        DevCommand::Sync { source } => cmd_sync(&source),
    }
}

fn cmd_sync(source: &str) -> Result<()> {
    let src = Path::new(source).canonicalize()
        .with_context(|| format!("source path not found: {source}"))?;

    // Verify we're in a Lynx source tree.
    if !src.join("Cargo.toml").exists() || !src.join("themes").exists() {
        bail!(
            "'{source}' does not look like a Lynx source tree \
             (expected Cargo.toml and themes/ directory)"
        );
    }

    let lynx_dir = lynx_dir();
    if !lynx_dir.exists() {
        bail!(
            "LYNX_DIR not installed at {} — run `lx setup` first",
            lynx_dir.display()
        );
    }

    // Step 1: Rebuild the binary.
    println!("building lx (release)...");
    let build = Command::new("cargo")
        .args(["build", "--release", "-p", "lynx-cli"])
        .current_dir(&src)
        .status()
        .context("failed to run cargo build")?;
    if !build.success() {
        bail!("cargo build failed — fix errors and retry");
    }

    // Step 2: Install the binary.
    let built_bin = src.join("target/release/lx");
    if built_bin.exists() {
        let current_bin = std::env::current_exe()
            .context("cannot determine current binary path")?;
        std::fs::copy(&built_bin, &current_bin)
            .with_context(|| format!(
                "failed to copy binary to {}",
                current_bin.display()
            ))?;
        println!("  updated: {} (binary)", current_bin.display());
    }

    // Step 3: Sync asset directories.
    let mut synced = 0usize;

    let sync_pairs: &[(&str, std::path::PathBuf)] = &[
        ("themes",   themes_dir()),
        ("shell",    lynx_dir.join("shell")),
        ("plugins",  lynx_dir.join("plugins")),
        ("contexts", lynx_dir.join("contexts")),
        ("profiles", lynx_dir.join("profiles")),
    ];

    for (dir_name, dst) in sync_pairs {
        let dir_src = src.join(dir_name);
        if dir_src.exists() {
            synced += sync_dir(&dir_src, dst, dir_name)
                .with_context(|| format!("failed to sync {dir_name}/"))?;
        }
    }

    println!("dev sync: {synced} file(s) updated from {}", src.display());
    Ok(())
}

/// Copy all files from `src` into `dst`, creating dst if needed.
/// Returns the count of files actually written (skips identical content).
fn sync_dir(src: &Path, dst: &Path, label: &str) -> Result<usize> {
    std::fs::create_dir_all(dst)
        .with_context(|| format!("cannot create {label} dst dir: {}", dst.display()))?;

    let mut count = 0usize;
    for entry in walkdir(src)? {
        let rel = entry.strip_prefix(src).unwrap();
        let dest = dst.join(rel);

        if entry.is_dir() {
            std::fs::create_dir_all(&dest)?;
            continue;
        }

        let src_bytes = std::fs::read(&entry)
            .with_context(|| format!("read {}", entry.display()))?;

        // Skip if destination is already identical.
        if dest.exists() {
            if let Ok(dst_bytes) = std::fs::read(&dest) {
                if dst_bytes == src_bytes {
                    continue;
                }
            }
        }

        std::fs::write(&dest, &src_bytes)
            .with_context(|| format!("write {}", dest.display()))?;
        println!("  updated: {}", dest.display());
        count += 1;
    }

    Ok(count)
}

/// Flat recursive file list (dirs included so we can mkdir them).
fn walkdir(root: &Path) -> Result<Vec<std::path::PathBuf>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(root)
        .with_context(|| format!("read dir {}", root.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            out.push(path.clone());
            out.extend(walkdir(&path)?);
        } else {
            out.push(path);
        }
    }
    Ok(out)
}
