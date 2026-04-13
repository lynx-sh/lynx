use anyhow::Result;
use clap::Args;
use std::process::{Command, Stdio};

#[derive(Args)]
pub struct KubectlStateArgs {}

/// `lx kubectl-state` — gather kubectl context/namespace and emit zsh that sets
/// `_lynx_kubectl_state` and exports `LYNX_CACHE_KUBECTL_STATE` as JSON.
///
/// Called by `lx refresh-state` when the kubectl plugin is enabled.
/// Can also be called manually for debugging.
///
/// Output when kubectl is configured:
/// ```
/// _lynx_kubectl_state=(context 'staging' namespace 'web')
/// export LYNX_CACHE_KUBECTL_STATE='{"context":"staging","namespace":"web"}'
/// ```
///
/// Output when kubectl is unavailable or unconfigured:
/// ```
/// _lynx_kubectl_state=()
/// export LYNX_CACHE_KUBECTL_STATE=''
/// ```
pub fn run(_args: KubectlStateArgs) -> Result<()> {
    let state = gather_kubectl_state();
    print!("{}", render_zsh(&state));
    Ok(())
}

pub(crate) struct KubectlState {
    pub(crate) context: Option<String>,
    pub(crate) namespace: Option<String>,
}

/// Run a kubectl subcommand, capture stdout. Returns `None` on failure.
fn kubectl(args: &[&str]) -> Option<String> {
    let out = Command::new("kubectl")
        .args(args)
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if out.status.success() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    } else {
        None
    }
}

pub(crate) fn gather_kubectl_state() -> KubectlState {
    // Bail fast if kubectl is not on PATH
    if lynx_core::paths::find_binary("kubectl").is_none() {
        return KubectlState {
            context: None,
            namespace: None,
        };
    }

    // Bail if no kubeconfig exists
    let home = std::env::var("HOME").unwrap_or_default();
    let kubeconfig_env = std::env::var("KUBECONFIG").ok();
    let default_kube = format!("{home}/.kube/config");
    let has_config = kubeconfig_env
        .as_deref()
        .map(|p| std::path::Path::new(p).exists())
        .unwrap_or_else(|| std::path::Path::new(&default_kube).exists());

    if !has_config {
        return KubectlState {
            context: None,
            namespace: None,
        };
    }

    let context = kubectl(&["config", "current-context"]);
    let namespace = kubectl(&[
        "config",
        "view",
        "--minify",
        "--output",
        "jsonpath={..namespace}",
    ])
    .or_else(|| Some("default".to_string()));

    KubectlState { context, namespace }
}

pub(crate) fn render_zsh(state: &KubectlState) -> String {
    match &state.context {
        None => "_lynx_kubectl_state=()\nexport LYNX_CACHE_KUBECTL_STATE=''\n".to_string(),
        Some(ctx) => {
            let ns = state.namespace.as_deref().unwrap_or("default");
            let ctx_esc = ctx.replace('\'', "'\\''");
            let ns_esc = ns.replace('\'', "'\\''");
            let ctx_json = ctx.replace('\\', "\\\\").replace('"', "\\\"");
            let ns_json = ns.replace('\\', "\\\\").replace('"', "\\\"");
            let json = format!(r#"{{"context":"{ctx_json}","namespace":"{ns_json}"}}"#);
            format!(
                "_lynx_kubectl_state=(context '{ctx_esc}' namespace '{ns_esc}')\nexport LYNX_CACHE_KUBECTL_STATE='{json}'\n"
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_clears_when_no_context() {
        let state = KubectlState {
            context: None,
            namespace: None,
        };
        let out = render_zsh(&state);
        assert!(out.contains("_lynx_kubectl_state=()"));
        assert!(out.contains("export LYNX_CACHE_KUBECTL_STATE=''"));
    }

    #[test]
    fn render_sets_context_and_namespace() {
        let state = KubectlState {
            context: Some("staging".into()),
            namespace: Some("web".into()),
        };
        let out = render_zsh(&state);
        assert!(out.contains("context 'staging'"));
        assert!(out.contains("namespace 'web'"));
        assert!(out.contains(r#""context":"staging""#));
        assert!(out.contains(r#""namespace":"web""#));
    }

    #[test]
    fn render_defaults_namespace_to_default() {
        let state = KubectlState {
            context: Some("prod".into()),
            namespace: None,
        };
        let out = render_zsh(&state);
        assert!(out.contains("namespace 'default'"));
        assert!(out.contains(r#""namespace":"default""#));
    }

    #[test]
    fn render_exports_json_cache() {
        let state = KubectlState {
            context: Some("my-cluster".into()),
            namespace: Some("api".into()),
        };
        let out = render_zsh(&state);
        assert!(out.contains("export LYNX_CACHE_KUBECTL_STATE='"));
        assert!(out.contains(r#""context":"my-cluster""#));
    }
}
