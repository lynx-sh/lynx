use crate::schema::{PluginManifest, CURRENT_SCHEMA_VERSION};
use lynx_core::error::{LynxError, Result};

/// Validates a parsed [`PluginManifest`].
///
/// Returns `Ok(())` if valid, or the first `LynxError::Manifest` describing the violation.
/// Note: binary PATH checks are done here; plugin dep resolution lives in lynx-depgraph.
pub fn validate(manifest: &PluginManifest) -> Result<()> {
    check_schema_version(manifest)?;
    check_no_wildcard_exports(manifest)?;
    check_shell_identifiers(manifest)?;
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
                "wildcard export '{name}' is not allowed — list names explicitly"
            )));
        }
    }
    Ok(())
}

fn check_binary_deps(manifest: &PluginManifest) -> Result<()> {
    for bin in &manifest.deps.binaries {
        if which(bin).is_none() {
            return Err(LynxError::Manifest(format!(
                "required binary '{bin}' not found on PATH"
            )));
        }
    }
    Ok(())
}

fn check_shell_identifiers(manifest: &PluginManifest) -> Result<()> {
    ensure_plugin_name("plugin.name", &manifest.plugin.name)?;
    for bin in &manifest.deps.binaries {
        ensure_command_name("deps.binaries", bin)?;
    }
    for hook in &manifest.load.hooks {
        ensure_hook_name("load.hooks", hook)?;
    }
    for widget in &manifest.shell.widgets {
        ensure_widget_name("shell.widgets", widget)?;
    }
    for keybinding in &manifest.shell.keybindings {
        ensure_widget_name("shell.keybindings.widget", &keybinding.widget)?;
    }
    Ok(())
}

fn ensure_plugin_name(field: &str, value: &str) -> Result<()> {
    if !is_plugin_name(value) {
        return Err(LynxError::Manifest(format!(
            "invalid {field} value '{value}' — expected [A-Za-z0-9_-]+"
        )));
    }
    Ok(())
}

fn ensure_command_name(field: &str, value: &str) -> Result<()> {
    if !is_command_name(value) {
        return Err(LynxError::Manifest(format!(
            "invalid {field} value '{value}' — expected [A-Za-z0-9_.+-]+"
        )));
    }
    Ok(())
}

fn ensure_hook_name(field: &str, value: &str) -> Result<()> {
    if !is_hook_name(value) {
        return Err(LynxError::Manifest(format!(
            "invalid {field} value '{value}' — expected [A-Za-z0-9_]+"
        )));
    }
    Ok(())
}

fn ensure_widget_name(field: &str, value: &str) -> Result<()> {
    if !is_widget_name(value) {
        return Err(LynxError::Manifest(format!(
            "invalid {field} value '{value}' — expected [A-Za-z0-9_-]+"
        )));
    }
    Ok(())
}

fn is_plugin_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn is_command_name(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|ch| {
            ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.' || ch == '+'
        })
}

fn is_hook_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn is_widget_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
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
            state: StateConfig::default(),
            shell: ShellConfig::default(),
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

    #[test]
    fn invalid_plugin_name_rejected() {
        let mut m = base_manifest();
        m.plugin.name = "bad$name".into();
        let err = validate(&m).unwrap_err();
        assert!(err.to_string().contains("plugin.name"));
    }

    #[test]
    fn invalid_binary_identifier_rejected() {
        let mut m = base_manifest();
        m.deps.binaries = vec!["git;rm".into()];
        let err = validate(&m).unwrap_err();
        assert!(err.to_string().contains("deps.binaries"));
    }

    #[test]
    fn invalid_hook_identifier_rejected() {
        let mut m = base_manifest();
        m.load.hooks = vec!["precmd$".into()];
        let err = validate(&m).unwrap_err();
        assert!(err.to_string().contains("load.hooks"));
    }

    #[test]
    fn invalid_widget_identifier_rejected() {
        let mut m = base_manifest();
        m.shell.widgets = vec!["widget()".into()];
        let err = validate(&m).unwrap_err();
        assert!(err.to_string().contains("shell.widgets"));
    }

    #[test]
    fn invalid_keybinding_widget_identifier_rejected() {
        let mut m = base_manifest();
        m.shell.keybindings = vec![KeyBinding {
            key: "^R".into(),
            widget: "bad widget".into(),
        }];
        let err = validate(&m).unwrap_err();
        assert!(err.to_string().contains("shell.keybindings.widget"));
    }
}
