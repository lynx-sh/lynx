//! `lx browse` — categorized package listing from all taps.

use anyhow::Result;
use clap::Args;
use lynx_config::load as load_config;
use lynx_core::paths;
use lynx_registry::schema::PackageType;
use lynx_registry::tap::{load_taps, merge_tap_indexes, TappedEntry};

#[derive(Args)]
pub struct BrowseArgs {
    /// Filter by category (e.g. "file-management", "security")
    pub category: Option<String>,
    /// Filter by package type
    #[arg(long, value_parser = parse_type)]
    pub r#type: Option<PackageType>,
    /// Show only installed packages
    #[arg(long)]
    pub installed: bool,
}

fn parse_type(s: &str) -> Result<PackageType, String> {
    match s {
        "plugin" => Ok(PackageType::Plugin),
        "tool" => Ok(PackageType::Tool),
        "theme" => Ok(PackageType::Theme),
        "intro" => Ok(PackageType::Intro),
        "bundle" => Ok(PackageType::Bundle),
        other => Err(format!("unknown type '{other}' — use plugin, tool, theme, intro, or bundle")),
    }
}

pub async fn run(args: BrowseArgs) -> Result<()> {
    let taps_path = paths::taps_config_path();
    let list = load_taps(&taps_path)?;
    let merged = merge_tap_indexes(&list)?;

    let config = load_config().ok();
    let enabled: Vec<String> = config
        .map(|c| c.enabled_plugins)
        .unwrap_or_default();

    // Apply filters.
    let filtered: Vec<&TappedEntry> = merged
        .iter()
        .filter(|t| {
            if let Some(ref cat) = args.category {
                if !t.entry.category.eq_ignore_ascii_case(cat) {
                    return false;
                }
            }
            if let Some(ref pkg_type) = args.r#type {
                if &t.entry.package_type != pkg_type {
                    return false;
                }
            }
            if args.installed && !enabled.contains(&t.entry.name) {
                return false;
            }
            true
        })
        .collect();

    if filtered.is_empty() {
        println!("no packages found");
        return Ok(());
    }

    // Group by category.
    let mut categories: std::collections::BTreeMap<String, Vec<&TappedEntry>> =
        std::collections::BTreeMap::new();
    for entry in &filtered {
        let cat = if entry.entry.category.is_empty() {
            "uncategorized".to_string()
        } else {
            entry.entry.category.clone()
        };
        categories.entry(cat).or_default().push(entry);
    }

    for (category, entries) in &categories {
        println!("\n  \x1b[1;34m{}\x1b[0m", category);
        println!("  {}", "-".repeat(60));
        for t in entries {
            let installed_mark = if enabled.contains(&t.entry.name) {
                "\x1b[32m✓\x1b[0m"
            } else {
                " "
            };
            let type_label = match t.entry.package_type {
                PackageType::Plugin => "plugin",
                PackageType::Tool => "tool",
                PackageType::Theme => "theme",
                PackageType::Intro => "intro",
                PackageType::Bundle => "bundle",
            };
            println!(
                "  {}{} {:<20} {:<8} {} {}",
                installed_mark,
                t.trust.badge(),
                t.entry.name,
                type_label,
                t.entry.description,
                if t.entry.theme_integrated { "(themed)" } else { "" }
            );
        }
    }
    println!();
    println!("  install: lx install <name>   ✓ = installed   {} = official", "✓");
    Ok(())
}
