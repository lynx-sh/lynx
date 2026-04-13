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

    let config = match load_config() {
        Ok(c) => Some(c),
        Err(e) => {
            lynx_core::diag::warn("browse", &format!("failed to load config — installed filter may be incomplete: {e}"));
            None
        }
    };
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

    let browse_entries: Vec<BrowseListEntry> = filtered
        .iter()
        .map(|t| {
            let type_label = match t.entry.package_type {
                PackageType::Plugin => "plugin",
                PackageType::Tool => "tool",
                PackageType::Theme => "theme",
                PackageType::Intro => "intro",
                PackageType::Bundle => "bundle",
                PackageType::Workflow => "workflow",
            };
            BrowseListEntry {
                name: t.entry.name.clone(),
                description: t.entry.description.clone(),
                type_label: type_label.to_string(),
                category: t.entry.category.clone(),
                tap: t.tap_name.clone(),
                installed: enabled.contains(&t.entry.name),
                themed: t.entry.theme_integrated,
            }
        })
        .collect();

    let tui_colors = match load_config() {
        Ok(cfg) => match lynx_theme::loader::load(&cfg.active_theme) {
            Ok(theme) => lynx_tui::TuiColors::from_palette(&theme.colors),
            Err(_) => lynx_tui::TuiColors::default(),
        },
        Err(_) => lynx_tui::TuiColors::default(),
    };

    if let Some(idx) = lynx_tui::show(&browse_entries, "Registry", &tui_colors)? {
        let entry = &browse_entries[idx];
        if !entry.installed {
            println!("  Install: lx install {}", entry.name);
        } else {
            println!("  Already installed: {}", entry.name);
        }
    }

    Ok(())
}

struct BrowseListEntry {
    name: String,
    description: String,
    type_label: String,
    category: String,
    tap: String,
    installed: bool,
    themed: bool,
}

impl lynx_tui::ListItem for BrowseListEntry {
    fn title(&self) -> &str {
        &self.name
    }
    fn subtitle(&self) -> String {
        self.description.clone()
    }
    fn detail(&self) -> String {
        let mut lines = vec![
            self.description.clone(),
            String::new(),
            format!("Type: {}", self.type_label),
            format!("Category: {}", self.category),
            format!("Tap: {}", self.tap),
        ];
        if self.themed {
            lines.push("Theme integrated: yes".to_string());
        }
        if self.installed {
            lines.push(String::new());
            lines.push("Status: installed".to_string());
        }
        lines.join("\n")
    }
    fn category(&self) -> Option<&str> {
        Some(&self.category)
    }
    fn tags(&self) -> Vec<&str> {
        vec![&self.type_label, &self.category, &self.tap]
    }
    fn is_active(&self) -> bool {
        self.installed
    }
}
