use anyhow::Result;
use clap::{Args, Subcommand};
use lynx_core::error::LynxError;
use lynx_core::paths::taps_config_path;
use lynx_registry::tap::{add_tap, load_taps, remove_tap, resolve_tap_url, save_taps};

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub struct TapArgs {
    #[command(subcommand)]
    pub command: TapCommand,
}

#[derive(Subcommand)]
pub enum TapCommand {
    /// List all configured taps
    List,
    /// Add a community tap (GitHub shorthand or full URL)
    Add {
        /// Tap source: "user/repo" or full URL to index.toml
        source: String,
    },
    /// Remove a tap
    Remove {
        /// Tap name to remove
        name: String,
    },
    /// Refresh all tap indexes
    Update,
    /// Catch unknown subcommands for friendly error
    #[command(external_subcommand)]
    Other(Vec<String>),
}

pub fn run(args: TapArgs) -> Result<()> {
    match args.command {
        TapCommand::List => cmd_list(),
        TapCommand::Add { source } => cmd_add(&source),
        TapCommand::Remove { name } => cmd_remove(&name),
        TapCommand::Update => cmd_update(),
        TapCommand::Other(args) => {
            Err(LynxError::unknown_command(super::unknown_subcmd_name(&args), "tap").into())
        }
    }
}

struct TapListEntry {
    name: String,
    url: String,
    trust: String,
}

impl lynx_tui::ListItem for TapListEntry {
    fn title(&self) -> &str {
        &self.name
    }
    fn subtitle(&self) -> String {
        self.trust.clone()
    }
    fn detail(&self) -> String {
        format!("URL: {}\nTrust: {}", self.url, self.trust)
    }
    fn category(&self) -> Option<&str> {
        Some("tap")
    }
}

fn cmd_list() -> Result<()> {
    let path = taps_config_path();
    let list = load_taps(&path)?;

    let entries: Vec<TapListEntry> = list
        .taps
        .iter()
        .map(|tap| TapListEntry {
            name: tap.name.clone(),
            url: tap.url.clone(),
            trust: tap.trust.label().to_string(),
        })
        .collect();

    lynx_tui::show(&entries, "Taps", &super::tui_colors())?;
    Ok(())
}

/// Derive a tap name from a source string.
/// - `"user/repo"` shorthand → `"user/repo"`
/// - GitHub URL (github.com or raw.githubusercontent.com) → `"user/repo"` from path segments 0+1
/// - Other full URL → `"host/last-segment"` (file extension stripped)
fn extract_tap_name(source: &str) -> String {
    if !source.starts_with("http") {
        return source.to_string();
    }

    // Strip scheme to get "host/path..."
    let without_scheme = source
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');

    let parts: Vec<&str> = without_scheme
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();

    let host = parts.first().copied().unwrap_or("");
    let path_parts = &parts[1..];

    // GitHub: github.com/user/repo/... or raw.githubusercontent.com/user/repo/...
    if (host == "github.com" || host == "raw.githubusercontent.com") && path_parts.len() >= 2 {
        return format!("{}/{}", path_parts[0], path_parts[1]);
    }

    // Fallback: host + last meaningful segment (strip file extension)
    if let Some(last) = path_parts.last() {
        let stem = last.split('.').next().unwrap_or(last);
        return format!("{host}/{stem}");
    }

    host.to_string()
}

fn cmd_add(source: &str) -> Result<()> {
    let path = taps_config_path();
    let mut list = load_taps(&path)?;

    let name = extract_tap_name(source);

    let url = resolve_tap_url(source);
    add_tap(&mut list, &name, &url)?;
    save_taps(&list, &path)?;

    println!("○ added tap '{name}' ({url})");
    println!("  run `lx tap update` to fetch the index");
    Ok(())
}

fn cmd_remove(name: &str) -> Result<()> {
    let path = taps_config_path();
    let mut list = load_taps(&path)?;
    remove_tap(&mut list, name)?;
    save_taps(&list, &path)?;
    println!("removed tap '{name}'");
    Ok(())
}

fn cmd_update() -> Result<()> {
    let path = taps_config_path();
    let list = load_taps(&path)?;

    let mut failures = 0u32;
    let total = list.taps.len();

    for tap in &list.taps {
        print!("{} {} ... ", tap.trust.badge(), tap.name);
        match ureq::get(&tap.url).call() {
            Ok(resp) if resp.status() < 400 => {
                let cache_dir = lynx_core::paths::registry_cache_dir();
                if let Err(e) = std::fs::create_dir_all(&cache_dir) {
                    println!("failed (cannot create cache dir: {e})");
                    failures += 1;
                    continue;
                }
                let cache_file = cache_dir.join(format!("{}.toml", tap.name.replace('/', "_")));
                match resp.into_string() {
                    Ok(body) => {
                        if let Err(e) = std::fs::write(&cache_file, &body) {
                            println!("failed (cannot write cache: {e})");
                            failures += 1;
                        } else {
                            println!("ok");
                        }
                    }
                    Err(e) => {
                        println!("failed (cannot read response: {e})");
                        failures += 1;
                    }
                }
            }
            Ok(resp) => {
                println!("error (status {})", resp.status());
                failures += 1;
            }
            Err(e) => {
                println!("failed ({e})");
                failures += 1;
            }
        }
    }

    if failures > 0 && failures == total as u32 {
        return Err(LynxError::Registry(format!("all {total} tap(s) failed to update")).into());
    } else if failures > 0 {
        eprintln!("warning: {failures}/{total} tap(s) failed to update");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tap_name_from_github_shorthand() {
        assert_eq!(extract_tap_name("user/repo"), "user/repo");
    }

    #[test]
    fn tap_name_from_full_github_url() {
        assert_eq!(
            extract_tap_name("https://github.com/user/repo/raw/main/index.toml"),
            "user/repo"
        );
    }

    #[test]
    fn tap_name_from_raw_githubusercontent_url() {
        assert_eq!(
            extract_tap_name("https://raw.githubusercontent.com/user/repo/main/index.toml"),
            "user/repo"
        );
    }

    #[test]
    fn tap_name_from_other_url_strips_extension() {
        assert_eq!(
            extract_tap_name("https://example.com/taps/community/index.toml"),
            "example.com/index"
        );
    }

    #[test]
    fn tap_name_from_url_with_trailing_slash() {
        assert_eq!(
            extract_tap_name("https://example.com/taps/community/"),
            "example.com/community"
        );
    }

    #[test]
    fn tap_list_entry_trait() {
        use lynx_tui::ListItem;
        let entry = TapListEntry {
            name: "core".to_string(),
            url: "https://example.com/index.toml".to_string(),
            trust: "official".to_string(),
        };
        assert_eq!(entry.title(), "core");
        assert_eq!(entry.subtitle(), "official");
        assert!(entry.detail().contains("https://example.com"));
        assert_eq!(entry.category(), Some("tap"));
    }

    #[tokio::test]
    async fn tap_unknown_subcommand_errors() {
        let args = TapArgs {
            command: TapCommand::Other(vec!["nope".to_string()]),
        };
        let err = run(args).unwrap_err();
        assert!(err.to_string().contains("nope"));
    }
}
