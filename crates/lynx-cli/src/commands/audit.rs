//! `lx audit` — security and transparency for enabled plugins.
//!
//! Shows what each package exports, hooks into, and accesses.

use anyhow::Result;
use clap::Args;
use lynx_config::load as load_config;
use lynx_core::paths;
use lynx_tui::ListItem;

#[derive(Args)]
pub struct AuditArgs {
    /// Show detailed info for a specific plugin
    pub name: Option<String>,
}

struct AuditEntry {
    name: String,
    source: String,
    context: String,
    binaries: String,
    fn_count: usize,
    alias_count: usize,
    description: String,
    version: String,
    authors: String,
    functions: Vec<String>,
    aliases: Vec<String>,
    hooks: String,
    lazy: bool,
    disabled_in: String,
    checksum_status: String,
}

impl lynx_tui::ListItem for AuditEntry {
    fn title(&self) -> &str {
        &self.name
    }

    fn subtitle(&self) -> String {
        format!("{} · {} fn, {} alias · {}", self.source, self.fn_count, self.alias_count, self.context)
    }

    fn detail(&self) -> String {
        let mut lines = vec![
            format!("Name:        {}", self.name),
            format!("Version:     {}", self.version),
            format!("Description: {}", self.description),
            format!("Authors:     {}", self.authors),
            String::new(),
            format!("Source:      {}", self.source),
            format!("Binaries:    {}", self.binaries),
            format!("Context:     {}", self.context),
            format!("Lazy:        {}", self.lazy),
            format!("Hooks:       {}", self.hooks),
            format!("Disabled in: {}", self.disabled_in),
            String::new(),
            "Exports:".to_string(),
        ];

        if self.functions.is_empty() {
            lines.push("  functions: none".to_string());
        } else {
            for f in &self.functions {
                lines.push(format!("  fn  {f}"));
            }
        }
        if self.aliases.is_empty() {
            lines.push("  aliases:   none".to_string());
        } else {
            for a in &self.aliases {
                lines.push(format!("  alias {a}"));
            }
        }

        if !self.checksum_status.is_empty() {
            lines.push(String::new());
            lines.push(format!("Checksum:    {}", self.checksum_status));
        }

        lines.join("\n")
    }

    fn category(&self) -> Option<&str> {
        Some("plugin")
    }
}

pub async fn run(args: AuditArgs) -> Result<()> {
    let config = load_config()?;
    let plugins_dir = paths::installed_plugins_dir();

    if config.enabled_plugins.is_empty() {
        println!("no plugins enabled");
        return Ok(());
    }

    let mut entries: Vec<AuditEntry> = Vec::new();

    for name in &config.enabled_plugins {
        let manifest_path = plugins_dir.join(name).join("plugin.toml");
        let (source, manifest) = match std::fs::read_to_string(&manifest_path) {
            Ok(content) => {
                let src = if manifest_path
                    .parent()
                    .and_then(|d| d.join(".auto-generated").exists().then_some(()))
                    .is_some()
                {
                    "auto"
                } else {
                    "bundled"
                };
                match lynx_manifest::parse_and_validate(&content) {
                    Ok(m) => (src.to_string(), Some(m)),
                    Err(_) => ("invalid".to_string(), None),
                }
            }
            Err(_) => ("missing".to_string(), None),
        };

        let entry = if let Some(manifest) = manifest {
            let p = &manifest.plugin;
            let disabled_in = manifest.contexts.disabled_in.join(", ");
            let context = if disabled_in.is_empty() {
                "all".to_string()
            } else {
                format!("!{disabled_in}")
            };

            let checksum_status = compute_checksum_status(name, &plugins_dir);

            AuditEntry {
                name: p.name.clone(),
                source,
                context,
                binaries: if manifest.deps.binaries.is_empty() {
                    "none".to_string()
                } else {
                    manifest.deps.binaries.join(", ")
                },
                fn_count: manifest.exports.functions.len(),
                alias_count: manifest.exports.aliases.len(),
                description: p.description.clone(),
                version: p.version.clone(),
                authors: p.authors.join(", "),
                functions: manifest.exports.functions.clone(),
                aliases: manifest.exports.aliases.clone(),
                hooks: if manifest.load.hooks.is_empty() {
                    "none".to_string()
                } else {
                    manifest.load.hooks.join(", ")
                },
                lazy: manifest.load.lazy,
                disabled_in: if disabled_in.is_empty() {
                    "none (loads everywhere)".to_string()
                } else {
                    disabled_in
                },
                checksum_status,
            }
        } else {
            AuditEntry {
                name: name.clone(),
                source,
                context: "?".to_string(),
                binaries: "?".to_string(),
                fn_count: 0,
                alias_count: 0,
                description: String::new(),
                version: String::new(),
                authors: String::new(),
                functions: vec![],
                aliases: vec![],
                hooks: "?".to_string(),
                lazy: false,
                disabled_in: "?".to_string(),
                checksum_status: String::new(),
            }
        };

        entries.push(entry);
    }

    // If a specific plugin was requested, filter to just that one and show detail.
    if let Some(ref name) = args.name {
        if let Some(entry) = entries.iter().find(|e| e.name == *name) {
            println!("{}", entry.detail());
            return Ok(());
        }
        return Err(lynx_core::error::LynxError::NotFound {
            item_type: "Plugin".into(),
            name: name.clone(),
            hint: "run `lx audit` to see enabled plugins".into(),
        }
        .into());
    }

    lynx_tui::show(&entries, "Plugin Audit", &super::tui_colors())?;
    Ok(())
}

fn compute_checksum_status(name: &str, plugins_dir: &std::path::Path) -> String {
    let Ok(lock) = lynx_registry::index::load_lock() else {
        return String::new();
    };
    let Some(locked) = lock.find(name) else {
        return "not tracked (bundled or local install)".to_string();
    };
    let Some(ref installed_hash) = locked.installed_checksum_sha256 else {
        return String::new();
    };
    match lynx_registry::fetch::checksum_plugin_dir(&plugins_dir.join(name)) {
        Ok(hash) if hash == *installed_hash => "✓ verified (matches lynx.lock)".to_string(),
        Ok(hash) => format!("⚠ MISMATCH — expected: {installed_hash}, actual: {hash}"),
        Err(_) => "? could not compute".to_string(),
    }
}
