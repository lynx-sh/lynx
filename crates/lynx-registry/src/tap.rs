//! Tap system — multi-registry support with trust tiers.
//!
//! A tap is a remote registry (GitHub repo with a registry.toml/index.toml).
//! Users can add community taps alongside the official lynx-sh/registry.
//! Each tap has a trust tier that is displayed alongside search results.

use std::path::Path;

use anyhow::{Context, Result};
use lynx_core::error::LynxError;
use serde::{Deserialize, Serialize};

use lynx_core::brand;

use crate::index::parse_index;
use crate::schema::{RegistryEntry, RegistryIndex};

// ── Trust tiers ─────────────────────────────────────────────────────────────

/// Trust level for a tap — determines the badge shown to users.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TrustTier {
    /// Curated by Lynx maintainers.
    #[default]
    Official,
    /// Passes automated validation.
    Verified,
    /// Unreviewed — user is warned before install.
    Community,
}

impl TrustTier {
    /// Display badge for CLI output.
    pub fn badge(&self) -> &'static str {
        match self {
            TrustTier::Official => "✓",
            TrustTier::Verified => "◆",
            TrustTier::Community => "○",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            TrustTier::Official => "official",
            TrustTier::Verified => "verified",
            TrustTier::Community => "community",
        }
    }
}

// ── Tap config ──────────────────────────────────────────────────────────────

/// A single tap configuration entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TapConfig {
    /// Short name (e.g. "official", "myuser/my-tools").
    pub name: String,
    /// URL to the raw index.toml file.
    pub url: String,
    /// Trust tier for this tap.
    #[serde(default)]
    pub trust: TrustTier,
}

/// The taps.toml file — list of all configured taps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TapList {
    #[serde(rename = "tap", default)]
    pub taps: Vec<TapConfig>,
}

/// Official tap — always present, cannot be removed.
fn official_tap() -> TapConfig {
    TapConfig {
        name: "official".to_string(),
        url: brand::DEFAULT_REGISTRY_URL.to_string(),
        trust: TrustTier::Official,
    }
}

// ── CRUD operations ─────────────────────────────────────────────────────────

/// Load taps from a taps.toml file. Ensures official tap is always present.
pub fn load_taps(path: &Path) -> Result<TapList> {
    let mut list = if path.exists() {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        toml::from_str::<TapList>(&content)
            .with_context(|| format!("failed to parse {}", path.display()))?
    } else {
        TapList::default()
    };
    ensure_official(&mut list);
    Ok(list)
}

/// Save taps to a taps.toml file.
pub fn save_taps(list: &TapList, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("failed to create taps dir")?;
    }
    let content = toml::to_string_pretty(list).context("failed to serialize taps")?;
    std::fs::write(path, content).context("failed to write taps.toml")?;
    Ok(())
}

/// Add a community tap. Returns error if name already exists.
pub fn add_tap(list: &mut TapList, name: &str, url: &str) -> Result<()> {
    if list.taps.iter().any(|t| t.name == name) {
        return Err(lynx_core::error::LynxError::Registry(format!("tap '{name}' already exists")).into());
    }
    list.taps.push(TapConfig {
        name: name.to_string(),
        url: url.to_string(),
        trust: TrustTier::Community,
    });
    Ok(())
}

/// Remove a tap by name. Refuses to remove the official tap.
pub fn remove_tap(list: &mut TapList, name: &str) -> Result<()> {
    if name == "official" {
        return Err(LynxError::Registry("cannot remove the official tap".into()).into());
    }
    let before = list.taps.len();
    list.taps.retain(|t| t.name != name);
    if list.taps.len() == before {
        return Err(LynxError::NotFound {
            item_type: "Tap".into(),
            name: name.into(),
            hint: "run `lx tap list` to see available taps".into(),
        }.into());
    }
    Ok(())
}

/// Ensure the official tap is always present in the list.
fn ensure_official(list: &mut TapList) {
    if !list.taps.iter().any(|t| t.name == "official") {
        list.taps.insert(0, official_tap());
    }
}

/// Resolve a GitHub shorthand (e.g. "user/repo") to a raw index URL.
pub fn resolve_tap_url(input: &str) -> String {
    if input.starts_with("http://") || input.starts_with("https://") {
        input.to_string()
    } else {
        // GitHub shorthand: user/repo → raw URL
        format!(
            "https://raw.githubusercontent.com/{input}/main/index.toml"
        )
    }
}

// ── Multi-tap index merge ───────────────────────────────────────────────────

/// A registry entry annotated with its source tap and trust tier.
#[derive(Debug, Clone)]
pub struct TappedEntry {
    pub entry: RegistryEntry,
    pub tap_name: String,
    pub trust: TrustTier,
}

/// Fetch and merge indexes from all taps.
/// Entries are deduplicated by name — highest trust tier wins.
pub fn merge_tap_indexes(list: &TapList) -> Result<Vec<TappedEntry>> {
    let mut merged: Vec<TappedEntry> = Vec::new();

    for tap in &list.taps {
        let idx = match fetch_tap_index(&tap.url) {
            Ok(idx) => idx,
            Err(e) => {
                tracing::warn!("failed to fetch tap '{}': {e}", tap.name);
                continue;
            }
        };

        for entry in idx.plugins {
            // Dedup: if name already exists, keep highest trust.
            if let Some(existing) = merged.iter().position(|t| t.entry.name == entry.name) {
                if trust_rank(&tap.trust) > trust_rank(&merged[existing].trust) {
                    merged[existing] = TappedEntry {
                        entry,
                        tap_name: tap.name.clone(),
                        trust: tap.trust.clone(),
                    };
                }
                // Lower or equal trust — skip.
            } else {
                merged.push(TappedEntry {
                    entry,
                    tap_name: tap.name.clone(),
                    trust: tap.trust.clone(),
                });
            }
        }
    }

    Ok(merged)
}

/// Merge from pre-loaded indexes (for testing or offline use).
pub fn merge_indexes(taps: &[(TapConfig, RegistryIndex)]) -> Vec<TappedEntry> {
    let mut merged: Vec<TappedEntry> = Vec::new();

    for (tap, idx) in taps {
        for entry in &idx.plugins {
            if let Some(existing) = merged.iter().position(|t| t.entry.name == entry.name) {
                if trust_rank(&tap.trust) > trust_rank(&merged[existing].trust) {
                    merged[existing] = TappedEntry {
                        entry: entry.clone(),
                        tap_name: tap.name.clone(),
                        trust: tap.trust.clone(),
                    };
                }
            } else {
                merged.push(TappedEntry {
                    entry: entry.clone(),
                    tap_name: tap.name.clone(),
                    trust: tap.trust.clone(),
                });
            }
        }
    }

    merged
}

fn trust_rank(tier: &TrustTier) -> u8 {
    match tier {
        TrustTier::Official => 3,
        TrustTier::Verified => 2,
        TrustTier::Community => 1,
    }
}

fn fetch_tap_index(url: &str) -> Result<RegistryIndex> {
    let resp = ureq::get(url)
        .call()
        .with_context(|| format!("HTTP GET failed for {url}"))?;
    if resp.status() >= 400 {
        return Err(LynxError::Registry(format!("registry returned status {} from {url}", resp.status())).into());
    }
    let body = resp.into_string().context("failed to read response")?;
    parse_index(&body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::PluginVersion;

    fn sample_entry(name: &str) -> RegistryEntry {
        RegistryEntry {
            name: name.into(),
            description: format!("{name} package"),
            latest_version: "1.0.0".into(),
            versions: vec![PluginVersion {
                version: "1.0.0".into(),
                url: "x".into(),
                checksum_sha256: "x".into(),
                min_lynx_version: None,
            }],
            ..Default::default()
        }
    }

    fn sample_index(names: &[&str]) -> RegistryIndex {
        RegistryIndex {
            plugins: names.iter().map(|n| sample_entry(n)).collect(),
        }
    }

    #[test]
    fn load_taps_creates_official_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("taps.toml");
        // File doesn't exist.
        let list = load_taps(&path).unwrap();
        assert_eq!(list.taps.len(), 1);
        assert_eq!(list.taps[0].name, "official");
        assert_eq!(list.taps[0].trust, TrustTier::Official);
    }

    #[test]
    fn load_taps_preserves_existing_and_adds_official() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("taps.toml");
        std::fs::write(&path, r#"
[[tap]]
name = "mytools"
url = "https://example.com/index.toml"
trust = "community"
"#).unwrap();
        let list = load_taps(&path).unwrap();
        assert_eq!(list.taps.len(), 2);
        assert_eq!(list.taps[0].name, "official");
        assert_eq!(list.taps[1].name, "mytools");
    }

    #[test]
    fn add_tap_succeeds() {
        let mut list = TapList::default();
        ensure_official(&mut list);
        add_tap(&mut list, "mytools", "https://example.com/index.toml").unwrap();
        assert_eq!(list.taps.len(), 2);
        assert_eq!(list.taps[1].trust, TrustTier::Community);
    }

    #[test]
    fn add_tap_rejects_duplicate() {
        let mut list = TapList::default();
        ensure_official(&mut list);
        add_tap(&mut list, "foo", "https://a.com").unwrap();
        assert!(add_tap(&mut list, "foo", "https://b.com").is_err());
    }

    #[test]
    fn remove_tap_succeeds() {
        let mut list = TapList::default();
        ensure_official(&mut list);
        add_tap(&mut list, "mytools", "https://example.com").unwrap();
        remove_tap(&mut list, "mytools").unwrap();
        assert_eq!(list.taps.len(), 1);
        assert_eq!(list.taps[0].name, "official");
    }

    #[test]
    fn remove_official_rejected() {
        let mut list = TapList::default();
        ensure_official(&mut list);
        assert!(remove_tap(&mut list, "official").is_err());
    }

    #[test]
    fn remove_nonexistent_rejected() {
        let mut list = TapList::default();
        ensure_official(&mut list);
        assert!(remove_tap(&mut list, "nope").is_err());
    }

    #[test]
    fn taps_toml_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("taps.toml");
        let mut list = TapList::default();
        ensure_official(&mut list);
        add_tap(&mut list, "community", "https://example.com/index.toml").unwrap();
        save_taps(&list, &path).unwrap();
        let loaded = load_taps(&path).unwrap();
        assert_eq!(list, loaded);
    }

    #[test]
    fn merge_indexes_deduplicates_by_trust() {
        let official_tap = TapConfig {
            name: "official".into(),
            url: "x".into(),
            trust: TrustTier::Official,
        };
        let community_tap = TapConfig {
            name: "community".into(),
            url: "y".into(),
            trust: TrustTier::Community,
        };

        // Both taps have "git", official should win.
        let official_idx = sample_index(&["git", "fzf"]);
        let community_idx = sample_index(&["git", "custom-tool"]);

        let merged = merge_indexes(&[
            (community_tap, community_idx),
            (official_tap, official_idx),
        ]);

        assert_eq!(merged.len(), 3); // git, custom-tool, fzf
        let git = merged.iter().find(|t| t.entry.name == "git").unwrap();
        assert_eq!(git.trust, TrustTier::Official);
        assert_eq!(git.tap_name, "official");
    }

    #[test]
    fn trust_tier_badges() {
        assert_eq!(TrustTier::Official.badge(), "✓");
        assert_eq!(TrustTier::Verified.badge(), "◆");
        assert_eq!(TrustTier::Community.badge(), "○");
    }

    #[test]
    fn resolve_github_shorthand() {
        let url = resolve_tap_url("myuser/my-tools");
        assert_eq!(url, "https://raw.githubusercontent.com/myuser/my-tools/main/index.toml");
    }

    #[test]
    fn resolve_full_url_unchanged() {
        let url = resolve_tap_url("https://example.com/custom.toml");
        assert_eq!(url, "https://example.com/custom.toml");
    }
}
