//! `lx audit` — security and transparency for enabled plugins.
//!
//! Shows what each package exports, hooks into, and accesses.

use anyhow::Result;
use clap::Args;
use lynx_config::load as load_config;
use lynx_core::paths;

#[derive(Args)]
pub struct AuditArgs {
    /// Show detailed info for a specific plugin
    pub name: Option<String>,
}

pub async fn run(args: AuditArgs) -> Result<()> {
    let config = load_config()?;
    let plugins_dir = paths::installed_plugins_dir();

    if let Some(name) = &args.name {
        return audit_one(name, &plugins_dir);
    }

    if config.enabled_plugins.is_empty() {
        println!("no plugins enabled");
        return Ok(());
    }

    println!(
        "{:<22} {:<10} {:<12} {:<10} EXPORTS",
        "NAME", "SOURCE", "CONTEXT", "BINARIES"
    );
    println!("{}", "-".repeat(72));

    for name in &config.enabled_plugins {
        let manifest_path = plugins_dir.join(name).join("plugin.toml");
        if let Ok(content) = std::fs::read_to_string(&manifest_path) {
            if let Ok(manifest) = lynx_manifest::parse_and_validate(&content) {
                let p = &manifest.plugin;
                let source = if manifest_path
                    .parent()
                    .and_then(|d| d.join(".auto-generated").exists().then_some(()))
                    .is_some()
                {
                    "auto"
                } else {
                    "bundled"
                };

                let bins = if manifest.deps.binaries.is_empty() {
                    "none".to_string()
                } else {
                    manifest.deps.binaries.join(", ")
                };

                let disabled_in = manifest
                    .contexts
                    .disabled_in
                    .join(", ");
                let context = if disabled_in.is_empty() {
                    "all".to_string()
                } else {
                    format!("!{disabled_in}")
                };

                let exports_count = manifest.exports.functions.len()
                    + manifest.exports.aliases.len();

                println!(
                    "{:<22} {:<10} {:<12} {:<10} {} fn, {} alias",
                    p.name,
                    source,
                    context,
                    bins,
                    manifest.exports.functions.len(),
                    manifest.exports.aliases.len(),
                );

                let _ = exports_count; // used above inline
            } else {
                println!("{:<22} {:<10} invalid manifest", name, "?");
            }
        } else {
            println!("{:<22} {:<10} no manifest found", name, "missing");
        }
    }

    Ok(())
}

fn audit_one(name: &str, plugins_dir: &std::path::Path) -> Result<()> {
    let manifest_path = plugins_dir.join(name).join("plugin.toml");
    let content = std::fs::read_to_string(&manifest_path)
        .map_err(|_| anyhow::anyhow!("plugin '{name}' not found at {}", manifest_path.display()))?;
    let manifest = lynx_manifest::parse_and_validate(&content)
        .map_err(|e| anyhow::anyhow!("invalid manifest for '{name}': {e}"))?;

    let p = &manifest.plugin;
    println!("name:        {}", p.name);
    println!("version:     {}", p.version);
    println!("description: {}", p.description);
    println!("authors:     {}", p.authors.join(", "));
    println!();

    println!("binary deps: {}", if manifest.deps.binaries.is_empty() {
        "none".to_string()
    } else {
        manifest.deps.binaries.join(", ")
    });

    println!("plugin deps: {}", if manifest.deps.plugins.is_empty() {
        "none".to_string()
    } else {
        manifest.deps.plugins.join(", ")
    });

    println!();
    println!("exports:");
    if manifest.exports.functions.is_empty() {
        println!("  functions: none");
    } else {
        for f in &manifest.exports.functions {
            println!("  fn  {f}");
        }
    }
    if manifest.exports.aliases.is_empty() {
        println!("  aliases:   none");
    } else {
        for a in &manifest.exports.aliases {
            println!("  alias {a}");
        }
    }

    println!();
    println!("context:");
    println!("  disabled_in: {}", if manifest.contexts.disabled_in.is_empty() {
        "none (loads everywhere)".to_string()
    } else {
        manifest.contexts.disabled_in.join(", ")
    });

    println!("  lazy: {}", manifest.load.lazy);
    println!("  hooks: {}", if manifest.load.hooks.is_empty() {
        "none".to_string()
    } else {
        manifest.load.hooks.join(", ")
    });

    // Checksum verification.
    if let Ok(lock) = lynx_registry::index::load_lock() {
        if let Some(locked) = lock.find(name) {
            if let Some(ref installed_hash) = locked.installed_checksum_sha256 {
                let current = lynx_registry::fetch::checksum_plugin_dir(
                    &plugins_dir.join(name),
                );
                match current {
                    Ok(hash) if hash == *installed_hash => {
                        println!("\nchecksum:    ✓ verified (matches lynx.lock)");
                    }
                    Ok(hash) => {
                        println!("\nchecksum:    ⚠ MISMATCH");
                        println!("  expected:  {installed_hash}");
                        println!("  actual:    {hash}");
                    }
                    Err(_) => {
                        println!("\nchecksum:    ? could not compute");
                    }
                }
            }
        } else {
            println!("\nchecksum:    not tracked (bundled or local install)");
        }
    }

    Ok(())
}
