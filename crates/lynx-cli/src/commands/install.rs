//! Unified package installer — `lx install` and `lx uninstall` (D-028).
//!
//! Resolves packages from all configured taps, detects the type,
//! and routes to the correct installer.

use anyhow::{Result};
use lynx_core::error::LynxError;
use clap::Args;
use lynx_config::snapshot::mutate_config_transaction;
use lynx_core::paths;
use lynx_registry::autoplug::generate_tool_plugin;
use lynx_registry::installer::{install_tool_via_pm, uninstall_tool};
use lynx_registry::schema::PackageType;
use lynx_registry::tap::{load_taps, merge_tap_indexes, TrustTier};

#[derive(Args)]
pub struct InstallPkgArgs {
    /// Package names to install
    pub names: Vec<String>,
    /// Force reinstall if already present
    #[arg(long)]
    pub force: bool,
}

#[derive(Args)]
pub struct UninstallPkgArgs {
    /// Package name to remove
    pub name: String,
}

pub async fn run_install(args: InstallPkgArgs) -> Result<()> {
    if args.names.is_empty() {
        return Err(LynxError::Registry("provide at least one package name — e.g. `lx install eza`".into()).into());
    }

    let taps_path = paths::taps_config_path();
    let list = load_taps(&taps_path)?;
    let merged = merge_tap_indexes(&list)?;

    for name in &args.names {
        let tapped = match merged.iter().find(|t| t.entry.name == *name) {
            Some(t) => t,
            None => {
                eprintln!("package '{name}' not found in any tap");
                continue;
            }
        };

        // Trust warning for community packages.
        if tapped.trust == TrustTier::Community {
            println!(
                "  {} '{name}' is from community tap '{}' — unverified",
                tapped.trust.badge(),
                tapped.tap_name
            );
            print!("  continue? [y/N] ");
            std::io::Write::flush(&mut std::io::stdout())?;
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("  skipped '{name}'");
                continue;
            }
        }

        let entry = &tapped.entry;
        match entry.package_type {
            PackageType::Tool => install_tool(name, entry, args.force)?,
            PackageType::Plugin => install_plugin(name, entry, args.force).await?,
            PackageType::Theme => {
                if entry.bundled {
                    println!("  ✓ theme '{name}' is bundled — use `lx theme set {name}`");
                } else {
                    let version = entry.resolve_version(None);
                    if let Some(v) = version {
                        lynx_registry::installer::install_theme(name, &v.url, args.force)?;
                        println!("  ✓ installed theme '{name}' — activate with `lx theme set {name}`");
                    } else {
                        println!("  no version found for theme '{name}'");
                    }
                }
            }
            PackageType::Intro => {
                if entry.bundled {
                    println!("  ✓ intro '{name}' is bundled — use `lx intro set {name}`");
                } else {
                    let version = entry.resolve_version(None);
                    if let Some(v) = version {
                        lynx_registry::installer::install_intro(name, &v.url, args.force)?;
                        println!("  ✓ installed intro '{name}'");
                    } else {
                        println!("  no version found for intro '{name}'");
                    }
                }
            }
            PackageType::Workflow => {
                let version = entry.resolve_version(None);
                if let Some(v) = version {
                    let dest = lynx_core::paths::workflows_dir().join(format!("{name}.toml"));
                    std::fs::create_dir_all(lynx_core::paths::workflows_dir())?;
                    let content = ureq::get(&v.url).call()?.into_string()?;
                    // Validate before writing
                    lynx_workflow::parse(&content)?;
                    std::fs::write(&dest, &content)?;
                    println!("  \u{2713} installed workflow '{name}' — run with `lx run {name}`");
                } else {
                    println!("  no version found for workflow '{name}'");
                }
            }
            PackageType::Bundle => {
                // Resolve bundle to its package list and install each.
                let all_entries: Vec<_> = merged.iter().map(|t| t.entry.clone()).collect();
                let idx = lynx_registry::schema::RegistryIndex { plugins: all_entries };
                let resolved = lynx_registry::bundle::resolve_bundle(entry, &idx)?;
                println!("  bundle '{name}' contains {} packages:", resolved.len());
                for pkg in &resolved {
                    println!("    - {}", pkg.name);
                }
                print!("  install all? [y/N] ");
                std::io::Write::flush(&mut std::io::stdout())?;
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("  skipped bundle '{name}'");
                    continue;
                }
                for pkg in &resolved {
                    println!();
                    match pkg.package_type {
                        PackageType::Tool => install_tool(&pkg.name, pkg, args.force)?,
                        PackageType::Plugin => install_plugin(&pkg.name, pkg, args.force).await?,
                        _ => println!("  skipping {} (type {:?})", pkg.name, pkg.package_type),
                    }
                }
                println!("  ✓ bundle '{name}' complete");
            }
        }
    }

    Ok(())
}

fn install_tool(
    name: &str,
    entry: &lynx_registry::schema::RegistryEntry,
    force: bool,
) -> Result<()> {
    let config = lynx_config::load()?;
    if config.enabled_plugins.contains(&name.to_string()) && !force {
        println!("  '{name}' is already installed — use --force to reinstall");
        return Ok(());
    }

    println!("  installing tool '{name}'...");
    install_tool_via_pm(entry)?;

    // Auto-generate plugin.
    let plugins_dir = paths::installed_plugins_dir();
    generate_tool_plugin(entry, &plugins_dir)?;

    // Enable the plugin.
    mutate_config_transaction(&format!("install-tool-{name}"), |cfg| {
        if !cfg.enabled_plugins.contains(&name.to_string()) {
            cfg.enabled_plugins.push(name.to_string());
        }
        Ok(())
    })?;

    println!("  ✓ installed '{name}' — restart your shell to activate");
    Ok(())
}

async fn install_plugin(
    name: &str,
    entry: &lynx_registry::schema::RegistryEntry,
    force: bool,
) -> Result<()> {
    if entry.bundled {
        mutate_config_transaction(&format!("install-plugin-{name}"), |cfg| {
            if !cfg.enabled_plugins.contains(&name.to_string()) {
                cfg.enabled_plugins.push(name.to_string());
            }
            Ok(())
        })?;
        println!("  ✓ enabled bundled plugin '{name}'");
        return Ok(());
    }

    use lynx_registry::fetch::{fetch_plugin, FetchOptions};
    let n = name.to_string();
    tokio::task::spawn_blocking(move || {
        fetch_plugin(
            &n,
            &FetchOptions {
                force,
                refresh_index: true,
                ..Default::default()
            },
        )
    })
    .await??;

    mutate_config_transaction(&format!("install-plugin-{name}"), |cfg| {
        if !cfg.enabled_plugins.contains(&name.to_string()) {
            cfg.enabled_plugins.push(name.to_string());
        }
        Ok(())
    })?;

    println!("  ✓ installed plugin '{name}'");
    Ok(())
}

pub async fn run_uninstall(args: UninstallPkgArgs) -> Result<()> {
    let name = &args.name;
    let plugins_dir = paths::installed_plugins_dir();
    let result = uninstall_tool(name, &plugins_dir)?;
    if result.plugin_removed {
        println!("removed Lynx plugin for '{name}'");
    }
    println!("system binary preserved — to remove it: {}", result.system_uninstall_hint);

    let config = lynx_config::load()?;
    if config.enabled_plugins.iter().any(|p| p == name) {
        mutate_config_transaction(&format!("uninstall-{name}"), |cfg| {
            cfg.enabled_plugins.retain(|p| p != name);
            Ok(())
        })?;
        println!("disabled '{name}' in config");
    }

    Ok(())
}
