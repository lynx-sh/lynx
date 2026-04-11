use crate::schema::{PluginManifest, CURRENT_SCHEMA_VERSION};
use lynx_core::error::{LynxError, Result};

/// Validates a parsed [`PluginManifest`].
///
/// Returns `Ok(())` if valid, or the first `LynxError::Manifest` describing the violation.
/// Note: binary PATH checks are done here; plugin dep resolution lives in lynx-depgraph.
pub fn validate(manifest: &PluginManifest) -> Result<()> {
    check_schema_version(manifest)?;
    check_no_wildcard_exports(manifest)?;
    check_binary_deps(manifest)?;
    Ok(())
}

fn check_schema_version(manifest: &PluginManifest) -> Result<()> {
    if manifest.schema_version != CURRENT_SCHEMA_VERSION {
        return Err(LynxError::Manifest(format!(
            "unsupported schema_version {} (expected {})",
            manifest.schema_version, CURRENT_SCHEMA_VERSION
        )));
    }
    Ok(())
}

fn check_no_wildcard_exports(manifest: &PluginManifest) -> Result<()> {
    let all = manifest
        .exports
        .functions
        .iter()
        .chain(manifest.exports.aliases.iter());
    for name in all {
        if name.contains('*') {
            return Err(LynxError::Manifest(format!(
                "wildcard export '{}' is not allowed — list names explicitly",
                name
            )));
        }
    }
    Ok(())
}

fn check_binary_deps(manifest: &PluginManifest) -> Result<()> {
    for bin in &manifest.deps.binaries {
        if which(bin).is_none() {
            return Err(LynxError::Manifest(format!(
                "required binary '{}' not found on PATH",
                bin
            )));
        }
    }
    Ok(())
}

fn which(bin: &str) -> Option<std::path::PathBuf> {
    lynx_core::paths::find_binary(bin)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::*;

    fn base_manifest() -> PluginManifest {
        PluginManifest {
            schema_version: CURRENT_SCHEMA_VERSION,
            plugin: PluginMeta {
                name: "test".into(),
                version: "0.1.0".into(),
                description: String::new(),
                authors: vec![],
            },
            load: LoadConfig::default(),
            deps: DepsConfig::default(),
            exports: ExportsConfig::default(),
            contexts: ContextsConfig::default(),
        }
    }

    #[test]
    fn valid_manifest_passes() {
        assert!(validate(&base_manifest()).is_ok());
    }

    #[test]
    fn wildcard_export_rejected() {
        let mut m = base_manifest();
        m.exports.functions = vec!["*".into()];
        let err = validate(&m).unwrap_err();
        assert!(err.to_string().contains("wildcard"));
    }

    #[test]
    fn missing_binary_dep_rejected() {
        let mut m = base_manifest();
        m.deps.binaries = vec!["__lynx_nonexistent_binary_xyz__".into()];
        let err = validate(&m).unwrap_err();
        assert!(err.to_string().contains("not found on PATH"));
    }

    #[test]
    fn wrong_schema_version_rejected() {
        let mut m = base_manifest();
        m.schema_version = 99;
        let err = validate(&m).unwrap_err();
        assert!(err.to_string().contains("schema_version"));
    }
}
