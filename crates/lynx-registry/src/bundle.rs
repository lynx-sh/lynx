//! Bundle support — install curated package collections.
//!
//! A bundle is a registry entry with `package_type = "bundle"` and a
//! `packages` list of other entry names. `resolve_bundle` expands the
//! list and validates that no nested bundles exist.

use anyhow::{bail, Result};

use crate::schema::{PackageType, RegistryEntry, RegistryIndex};

/// Resolve a bundle to its list of package entries.
/// Returns the entries in order, or an error if:
/// - any package in the bundle is not found in the index
/// - any package in the bundle is itself a bundle (no nesting)
pub fn resolve_bundle<'a>(
    bundle: &RegistryEntry,
    index: &'a RegistryIndex,
) -> Result<Vec<&'a RegistryEntry>> {
    if bundle.package_type != PackageType::Bundle {
        bail!("'{}' is not a bundle", bundle.name);
    }

    if bundle.packages.is_empty() {
        bail!("bundle '{}' has no packages listed", bundle.name);
    }

    let mut resolved = Vec::new();

    for name in &bundle.packages {
        let entry = index
            .find(name)
            .ok_or_else(|| anyhow::anyhow!(
                "bundle '{}' references package '{}' which is not in the index",
                bundle.name,
                name
            ))?;

        if entry.package_type == PackageType::Bundle {
            bail!(
                "bundle '{}' contains nested bundle '{}' — nesting is not allowed",
                bundle.name,
                name
            );
        }

        resolved.push(entry);
    }

    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::PluginVersion;

    fn entry(name: &str, pkg_type: PackageType) -> RegistryEntry {
        RegistryEntry {
            name: name.into(),
            description: format!("{name} pkg"),
            package_type: pkg_type,
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

    fn bundle_entry(name: &str, packages: &[&str]) -> RegistryEntry {
        RegistryEntry {
            name: name.into(),
            description: format!("{name} bundle"),
            package_type: PackageType::Bundle,
            packages: packages.iter().map(|s| s.to_string()).collect(),
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

    #[test]
    fn resolve_valid_bundle() {
        let idx = RegistryIndex {
            plugins: vec![
                entry("eza", PackageType::Tool),
                entry("bat", PackageType::Tool),
                entry("fd", PackageType::Tool),
                bundle_entry("modern-cli", &["eza", "bat", "fd"]),
            ],
        };
        let bundle = idx.find("modern-cli").unwrap();
        let resolved = resolve_bundle(bundle, &idx).unwrap();
        assert_eq!(resolved.len(), 3);
        assert_eq!(resolved[0].name, "eza");
        assert_eq!(resolved[1].name, "bat");
        assert_eq!(resolved[2].name, "fd");
    }

    #[test]
    fn rejects_nested_bundles() {
        let idx = RegistryIndex {
            plugins: vec![
                entry("eza", PackageType::Tool),
                bundle_entry("inner", &["eza"]),
                bundle_entry("outer", &["inner"]),
            ],
        };
        let bundle = idx.find("outer").unwrap();
        let result = resolve_bundle(bundle, &idx);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("nested bundle"));
    }

    #[test]
    fn rejects_missing_package() {
        let idx = RegistryIndex {
            plugins: vec![
                entry("eza", PackageType::Tool),
                bundle_entry("broken", &["eza", "nonexistent"]),
            ],
        };
        let bundle = idx.find("broken").unwrap();
        let result = resolve_bundle(bundle, &idx);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("nonexistent"));
    }

    #[test]
    fn rejects_non_bundle() {
        let idx = RegistryIndex {
            plugins: vec![entry("eza", PackageType::Tool)],
        };
        let tool = idx.find("eza").unwrap();
        assert!(resolve_bundle(tool, &idx).is_err());
    }

    #[test]
    fn rejects_empty_bundle() {
        let idx = RegistryIndex {
            plugins: vec![bundle_entry("empty", &[])],
        };
        let bundle = idx.find("empty").unwrap();
        assert!(resolve_bundle(bundle, &idx).is_err());
    }
}
