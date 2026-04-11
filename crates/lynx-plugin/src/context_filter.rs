use lynx_core::types::Context;
use lynx_manifest::schema::PluginManifest;

/// Filter a list of plugin manifests for the active context.
///
/// Returns `(allowed, disabled)` where `disabled` is a list of
/// `(plugin_name, reason)` pairs for plugins excluded by `disabled_in`.
pub fn filter_for_context(
    manifests: &[PluginManifest],
    context: &Context,
) -> (Vec<PluginManifest>, Vec<(String, String)>) {
    let ctx_str = context_str(context);
    let mut allowed = Vec::new();
    let mut disabled = Vec::new();

    for m in manifests {
        if m.contexts
            .disabled_in
            .iter()
            .any(|c| c.to_lowercase() == ctx_str)
        {
            disabled.push((
                m.plugin.name.clone(),
                format!("disabled in {} context", ctx_str),
            ));
        } else {
            allowed.push(m.clone());
        }
    }

    (allowed, disabled)
}

fn context_str(ctx: &Context) -> &'static str {
    match ctx {
        Context::Interactive => "interactive",
        Context::Agent => "agent",
        Context::Minimal => "minimal",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lynx_manifest::schema::*;

    fn git_manifest() -> PluginManifest {
        PluginManifest {
            schema_version: 1,
            plugin: PluginMeta {
                name: "git".into(),
                version: "0.1.0".into(),
                description: String::new(),
                authors: vec![],
            },
            load: LoadConfig::default(),
            deps: DepsConfig::default(),
            exports: ExportsConfig::default(),
            contexts: ContextsConfig {
                disabled_in: vec!["agent".into(), "minimal".into()],
            },
            state: StateConfig::default(),
            shell: ShellConfig::default(),
        }
    }

    #[test]
    fn agent_context_excludes_git_plugin() {
        let (allowed, disabled) = filter_for_context(&[git_manifest()], &Context::Agent);
        assert!(allowed.is_empty());
        assert_eq!(disabled.len(), 1);
        assert!(disabled[0].1.contains("agent"));
    }

    #[test]
    fn interactive_context_allows_git_plugin() {
        let (allowed, disabled) = filter_for_context(&[git_manifest()], &Context::Interactive);
        assert_eq!(allowed.len(), 1);
        assert!(disabled.is_empty());
    }

    #[test]
    fn plugin_list_shows_disabled_reason() {
        let (_, disabled) = filter_for_context(&[git_manifest()], &Context::Agent);
        assert!(disabled[0].1.contains("disabled in agent context"));
    }
}
