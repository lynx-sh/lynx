use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use lynx_core::brand;
use lynx_core::brand::ZSHRC_INIT_LINE;

#[derive(Args)]
pub struct InstallArgs {
    /// Also add `source ~/.config/lynx/shell/init.zsh` to ~/.zshrc
    #[arg(long)]
    pub zshrc: bool,

    /// Target install directory (default: ~/.config/lynx)
    #[arg(long)]
    pub dir: Option<String>,

    /// Source directory containing shell/ and plugins/ (default: detected from binary location)
    #[arg(long)]
    pub source: Option<String>,
}

pub async fn run(args: InstallArgs) -> Result<()> {
    let home = home_dir()?;

    let lynx_dir: PathBuf = args
        .dir
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(brand::CONFIG_DIR));

    let source_dir = resolve_source_dir(args.source.as_deref())?;

    println!("Installing {} to {}...", brand::NAME, lynx_dir.display());
    println!();

    // Snapshot existing config dir before any writes (D-007)
    snapshot_if_exists(&lynx_dir);

    // Create target directory
    std::fs::create_dir_all(&lynx_dir)
        .with_context(|| format!("cannot create install dir: {}", lynx_dir.display()))?;

    // Copy shell/ and plugins/
    let shell_src = source_dir.join("shell");
    let plugins_src = source_dir.join("plugins");
    let themes_src = source_dir.join("themes");

    if shell_src.exists() {
        copy_dir_all(&shell_src, &lynx_dir.join("shell"))
            .with_context(|| format!("failed to copy shell/ from {}", shell_src.display()))?;
        println!("  ✓ shell/   → {}/shell/", lynx_dir.display());
    } else {
        anyhow::bail!(
            "shell/ not found at {} — run lx install from the Lynx source directory or pass --source",
            shell_src.display()
        );
    }

    if plugins_src.exists() {
        copy_dir_all(&plugins_src, &lynx_dir.join("plugins"))
            .with_context(|| format!("failed to copy plugins/ from {}", plugins_src.display()))?;
        println!("  ✓ plugins/ → {}/plugins/", lynx_dir.display());
    } else {
        println!("  ⚠ plugins/ not found at {} — skipping", plugins_src.display());
    }

    if themes_src.exists() {
        let themes_dst = lynx_dir.join("themes");
        copy_dir_all(&themes_src, &themes_dst)
            .with_context(|| format!("failed to copy themes/ from {}", themes_src.display()))?;
        println!("  ✓ themes/  → {}/themes/", lynx_dir.display());
    } else {
        println!("  ⚠ themes/ not found — skipping");
    }

    // Write default config if none exists
    let config_path = lynx_dir.join(brand::CONFIG_FILE);
    if !config_path.exists() {
        std::fs::write(
            &config_path,
            default_config(),
        )
        .with_context(|| format!("failed to write config.toml to {}", config_path.display()))?;
        println!("  ✓ config.toml written (default)");
    } else {
        println!("  ✓ config.toml preserved (already exists)");
    }

    // Optionally patch .zshrc
    if args.zshrc {
        patch_zshrc(&home)?;
    }

    println!();
    println!("Lynx installed to {}.", lynx_dir.display());

    if !args.zshrc {
        println!();
        println!("Add this to your ~/.zshrc to activate Lynx:");
        println!();
        println!("    {ZSHRC_INIT_LINE}");
        println!();
        println!("Or run `lx install --zshrc` to do it automatically.");
    } else {
        println!();
        println!("Restart your shell or run:");
        println!();
        println!("    source ~/.zshrc");
    }

    Ok(())
}

/// Resolve the source directory: explicit --source, or CARGO_MANIFEST_DIR (dev), or binary location.
fn resolve_source_dir(explicit: Option<&str>) -> Result<PathBuf> {
    if let Some(s) = explicit {
        return Ok(PathBuf::from(s));
    }

    // In dev: CARGO_MANIFEST_DIR is set when running via cargo run.
    // Navigate up to workspace root (crates/lynx-cli → ../../)
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let workspace_root = PathBuf::from(&manifest)
            .parent() // lynx-cli
            .and_then(|p| p.parent()) // crates
            .and_then(|p| p.parent()) // workspace root
            .map(|p| p.to_path_buf());
        if let Some(root) = workspace_root {
            if root.join("shell").exists() {
                return Ok(root);
            }
        }
    }

    // Production: binary sits at ~/.local/bin/lx or /usr/local/bin/lx.
    // Source tree is expected alongside it at a sibling share/ dir, or in the same dir.
    if let Ok(exe) = std::env::current_exe() {
        // Try: <exe-parent>/../share/lynx
        if let Some(bin_dir) = exe.parent() {
            let share = bin_dir.join("../share/lynx");
            if share.join("shell").exists() {
                return Ok(share.canonicalize().unwrap_or(share));
            }
            // Try: <exe-parent> directly (portable bundle)
            if bin_dir.join("shell").exists() {
                return Ok(bin_dir.to_path_buf());
            }
        }
    }

    // Last resort: current working directory
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if cwd.join("shell").exists() {
        return Ok(cwd);
    }

    anyhow::bail!(
        "Cannot locate Lynx source files (shell/ and plugins/).\n\
         Run from the Lynx source directory or pass --source <path>."
    )
}

/// Snapshot the target directory to ~/.config/lynx/.snapshots/<timestamp> if it already exists.
fn snapshot_if_exists(lynx_dir: &Path) {
    if !lynx_dir.exists() {
        return;
    }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let snap_dir = lynx_dir.join(format!(".snapshots/install-{ts}"));
    if std::fs::create_dir_all(&snap_dir).is_ok() {
        // Best-effort: copy config.toml if it exists
        let cfg = lynx_dir.join("config.toml");
        if cfg.exists() {
            let _ = std::fs::copy(&cfg, snap_dir.join("config.toml"));
        }
        println!("  ✓ snapshot → {}", snap_dir.display());
    }
}

/// Recursively copy src into dst, creating dst if needed.
fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest_path)?;
        } else {
            std::fs::copy(entry.path(), &dest_path)
                .with_context(|| format!("failed to copy {}", entry.path().display()))?;
        }
    }
    Ok(())
}

/// Append the Lynx init line to ~/.zshrc if not already present.
fn patch_zshrc(home: &Path) -> Result<()> {
    let zshrc = home.join(".zshrc");

    if zshrc.exists() {
        let content = std::fs::read_to_string(&zshrc)
            .with_context(|| format!("failed to read {}", zshrc.display()))?;

        if content.lines().any(|l| l.contains(ZSHRC_INIT_LINE)) {
            println!("  ✓ ~/.zshrc already has Lynx init line — skipping");
            return Ok(());
        }

        // Append
        let mut appended = content;
        if !appended.ends_with('\n') {
            appended.push('\n');
        }
        appended.push_str(&format!("\n# Lynx shell framework\n{ZSHRC_INIT_LINE}\n"));
        std::fs::write(&zshrc, &appended)
            .with_context(|| format!("failed to write {}", zshrc.display()))?;
    } else {
        // Create minimal .zshrc
        std::fs::write(
            &zshrc,
            format!("# Lynx shell framework\n{ZSHRC_INIT_LINE}\n"),
        )
        .with_context(|| format!("failed to create {}", zshrc.display()))?;
    }

    println!("  ✓ ~/.zshrc patched with Lynx init line");
    Ok(())
}

/// Minimal default config.toml for a fresh install.
fn default_config() -> &'static str {
    r#"# Lynx configuration — https://github.com/lynx-sh/lynx

enabled_plugins = ["git", "kubectl"]

[theme]
name = "default"

[contexts.interactive]
enabled = true

[contexts.agent]
enabled = true

[contexts.minimal]
enabled = true
"#
}

fn home_dir() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("$HOME not set"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn copy_dir_all_copies_nested_files() {
        let src = TempDir::new().unwrap();
        let dst = TempDir::new().unwrap();

        fs::create_dir(src.path().join("sub")).unwrap();
        fs::write(src.path().join("file.txt"), b"hello").unwrap();
        fs::write(src.path().join("sub/nested.txt"), b"world").unwrap();

        copy_dir_all(src.path(), dst.path()).unwrap();

        assert!(dst.path().join("file.txt").exists());
        assert!(dst.path().join("sub/nested.txt").exists());
    }

    #[test]
    fn patch_zshrc_idempotent() {
        let home = TempDir::new().unwrap();
        let zshrc = home.path().join(".zshrc");
        fs::write(&zshrc, format!("{ZSHRC_INIT_LINE}\n")).unwrap();

        patch_zshrc(home.path()).unwrap();

        let content = fs::read_to_string(&zshrc).unwrap();
        assert_eq!(
            content.lines().filter(|l| l.contains(ZSHRC_INIT_LINE)).count(),
            1,
            "should not duplicate init line"
        );
    }

    #[test]
    fn patch_zshrc_creates_if_missing() {
        let home = TempDir::new().unwrap();
        patch_zshrc(home.path()).unwrap();
        let content = fs::read_to_string(home.path().join(".zshrc")).unwrap();
        assert!(content.contains(ZSHRC_INIT_LINE));
    }

    #[test]
    fn patch_zshrc_appends_to_existing() {
        let home = TempDir::new().unwrap();
        let zshrc = home.path().join(".zshrc");
        fs::write(&zshrc, "export FOO=bar\n").unwrap();

        patch_zshrc(home.path()).unwrap();

        let content = fs::read_to_string(&zshrc).unwrap();
        assert!(content.contains("export FOO=bar"));
        assert!(content.contains(ZSHRC_INIT_LINE));
    }
}
