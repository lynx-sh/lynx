use lynx_manifest::schema::PluginManifest;

/// A namespace violation: a name defined by the plugin but not declared in exports.
#[derive(Debug, Clone, PartialEq)]
pub struct NameViolation {
    pub plugin: String,
    pub name: String,
    pub kind: ViolationKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ViolationKind {
    /// Function defined but not in exports.functions.
    UnexportedFunction,
    /// Alias defined but not in exports.aliases.
    UnexportedAlias,
}

impl std::fmt::Display for NameViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match self.kind {
            ViolationKind::UnexportedFunction => "unexported function",
            ViolationKind::UnexportedAlias => "unexported alias",
        };
        write!(
            f,
            "[{}] {} '{}' is defined but not declared in exports",
            self.plugin, kind, self.name
        )
    }
}

/// Lint a plugin's actual defined names against its manifest exports.
///
/// The convention: internal helpers must use a `_` prefix.
/// Any non-`_`-prefixed name that isn't in exports is a violation.
///
/// `actual_functions` and `actual_aliases` are what the shell reports
/// after the plugin's init.zsh has been sourced.
pub fn lint_exports(
    manifest: &PluginManifest,
    actual_functions: &[String],
    actual_aliases: &[String],
) -> Vec<NameViolation> {
    let mut violations = Vec::new();
    let plugin = &manifest.plugin.name;

    for name in actual_functions {
        // Internal helpers (underscore prefix) are exempt
        if name.starts_with('_') {
            continue;
        }
        if !manifest.exports.functions.contains(name) {
            violations.push(NameViolation {
                plugin: plugin.clone(),
                name: name.clone(),
                kind: ViolationKind::UnexportedFunction,
            });
        }
    }

    for name in actual_aliases {
        if name.starts_with('_') {
            continue;
        }
        if !manifest.exports.aliases.contains(name) {
            violations.push(NameViolation {
                plugin: plugin.clone(),
                name: name.clone(),
                kind: ViolationKind::UnexportedAlias,
            });
        }
    }

    violations
}

/// Generate a plugin scaffold comment block for new plugins, documenting
/// the _ prefix convention for internal functions.
pub fn scaffold_convention_comment() -> &'static str {
    "# Internal helpers MUST use the _ prefix (e.g. _my_plugin_helper).\n\
     # Only names declared in plugin.toml exports.functions/aliases should\n\
     # be defined without the _ prefix."
}

#[cfg(test)]
mod tests {
    use super::*;
    use lynx_manifest::schema::*;

    fn manifest_with_exports(fns: &[&str], aliases: &[&str]) -> PluginManifest {
        PluginManifest {
            schema_version: 1,
            plugin: PluginMeta {
                name: "test-plugin".into(),
                version: "0.1.0".into(),
                description: String::new(),
                authors: vec![],
            },
            load: LoadConfig::default(),
            deps: DepsConfig::default(),
            exports: ExportsConfig {
                functions: fns.iter().map(|s| s.to_string()).collect(),
                aliases: aliases.iter().map(|s| s.to_string()).collect(),
            },
            contexts: ContextsConfig::default(),
        }
    }

    #[test]
    fn declared_export_passes() {
        let m = manifest_with_exports(&["git_branch"], &["gst"]);
        let v = lint_exports(&m, &["git_branch".into()], &["gst".into()]);
        assert!(v.is_empty());
    }

    #[test]
    fn unexported_function_flagged() {
        let m = manifest_with_exports(&["git_branch"], &[]);
        let v = lint_exports(&m, &["git_branch".into(), "secret_fn".into()], &[]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].name, "secret_fn");
        assert_eq!(v[0].kind, ViolationKind::UnexportedFunction);
    }

    #[test]
    fn underscore_prefix_internal_exempt() {
        let m = manifest_with_exports(&["git_branch"], &[]);
        let v = lint_exports(&m, &["git_branch".into(), "_internal_helper".into()], &[]);
        assert!(v.is_empty());
    }

    #[test]
    fn unexported_alias_flagged() {
        let m = manifest_with_exports(&[], &["gst"]);
        let v = lint_exports(&m, &[], &["gst".into(), "mystery_alias".into()]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].kind, ViolationKind::UnexportedAlias);
    }
}
