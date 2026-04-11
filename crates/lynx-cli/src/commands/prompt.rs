use anyhow::Result;
use clap::{Args, Subcommand};
use lynx_config::load as load_config;
use lynx_core::types::Context;
use lynx_prompt::{
    evaluator::evaluate_theme, renderer::render_prompt, segment::RenderContext, CmdDurationSegment,
    ContextBadgeSegment, DirSegment, GitBranchSegment, GitStatusSegment, KubectlContextSegment,
    ProfileBadgeSegment, TaskStatusSegment,
};
use lynx_theme::loader::load as load_theme;
use std::collections::HashMap;

#[derive(Args)]
pub struct PromptArgs {
    #[command(subcommand)]
    pub command: PromptCommand,
}

#[derive(Subcommand)]
pub enum PromptCommand {
    /// Render PROMPT and RPROMPT shell assignments for eval by precmd hook
    Render,
}

pub async fn run(args: PromptArgs) -> Result<()> {
    match args.command {
        PromptCommand::Render => cmd_render().await,
    }
}

async fn cmd_render() -> Result<()> {
    let ctx = build_render_context_from_env();

    // --- Load theme ---
    let theme_name = std::env::var("LYNX_THEME").unwrap_or_else(|_| "default".into());
    let theme = load_theme(&theme_name).or_else(|_| load_theme("default"))?;

    // --- Build segment registry ---
    let segments: Vec<Box<dyn lynx_prompt::segment::Segment>> = vec![
        Box::new(DirSegment),
        Box::new(GitBranchSegment),
        Box::new(GitStatusSegment),
        Box::new(KubectlContextSegment),
        Box::new(ProfileBadgeSegment),
        Box::new(TaskStatusSegment),
        Box::new(CmdDurationSegment),
        Box::new(ContextBadgeSegment),
    ];

    // --- Evaluate and render ---
    let (left, right) = evaluate_theme(&segments, &theme, &ctx).await;
    let output = render_prompt(&left, &right, &theme);
    print!("{}", output);
    Ok(())
}

fn build_render_context_from_env() -> RenderContext {
    let cwd = std::env::var("PWD").unwrap_or_else(|_| "/".into());
    let shell_context = std::env::var(lynx_core::env_vars::LYNX_CONTEXT)
        .ok()
        .and_then(|v| Context::from_str(&v))
        .unwrap_or(Context::Interactive);
    let last_cmd_ms = std::env::var("LYNX_LAST_CMD_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok());

    let mut cache: HashMap<String, serde_json::Value> = HashMap::new();

    if let Ok(git_json) = std::env::var("LYNX_CACHE_GIT_STATE") {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&git_json) {
            cache.insert("git_state".into(), v);
        }
    }

    if let Ok(kubectl_json) = std::env::var("LYNX_CACHE_KUBECTL_STATE") {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&kubectl_json) {
            cache.insert("kubectl_state".into(), v);
        }
    }

    if let Ok(config) = load_config() {
        if let Some(profile) = &config.active_profile {
            if !profile.is_empty() {
                cache.insert(
                    "profile_state".into(),
                    serde_json::json!({ "name": profile }),
                );
            }
        }
    }

    RenderContext {
        cwd,
        shell_context,
        last_cmd_ms,
        cache,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lynx_test_utils::temp_home;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn set_env_or_remove(key: &str, value: Option<&str>) {
        if let Some(v) = value {
            std::env::set_var(key, v);
        } else {
            std::env::remove_var(key);
        }
    }

    struct EnvGuard {
        saved: Vec<(String, Option<String>)>,
    }

    impl EnvGuard {
        fn new(keys: &[&str]) -> Self {
            let mut saved = Vec::new();
            for key in keys {
                saved.push(((*key).to_string(), std::env::var(key).ok()));
            }
            Self { saved }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, val) in &self.saved {
                set_env_or_remove(key, val.as_deref());
            }
        }
    }

    #[test]
    fn defaults_to_interactive_context() {
        let _lock = env_lock().lock().expect("lock");
        let _guard = EnvGuard::new(&["LYNX_CONTEXT", "PWD"]);
        std::env::remove_var("LYNX_CONTEXT");
        std::env::set_var("PWD", "/tmp/demo");

        let ctx = build_render_context_from_env();
        assert!(matches!(ctx.shell_context, Context::Interactive));
        assert_eq!(ctx.cwd, "/tmp/demo");
    }

    #[test]
    fn parses_agent_context() {
        let _lock = env_lock().lock().expect("lock");
        let _guard = EnvGuard::new(&["LYNX_CONTEXT", "PWD"]);
        std::env::set_var("LYNX_CONTEXT", "agent");
        std::env::set_var("PWD", "/");
        let ctx = build_render_context_from_env();
        assert!(matches!(ctx.shell_context, Context::Agent));
    }

    #[test]
    fn parses_last_command_ms_from_env() {
        let _lock = env_lock().lock().expect("lock");
        let _guard = EnvGuard::new(&["LYNX_LAST_CMD_MS", "PWD"]);
        std::env::set_var("LYNX_LAST_CMD_MS", "1234");
        std::env::set_var("PWD", "/");
        let ctx = build_render_context_from_env();
        assert_eq!(ctx.last_cmd_ms, Some(1234));
    }

    #[test]
    fn invalid_last_command_ms_is_ignored() {
        let _lock = env_lock().lock().expect("lock");
        let _guard = EnvGuard::new(&["LYNX_LAST_CMD_MS", "PWD"]);
        std::env::set_var("LYNX_LAST_CMD_MS", "not-a-number");
        std::env::set_var("PWD", "/");
        let ctx = build_render_context_from_env();
        assert_eq!(ctx.last_cmd_ms, None);
    }

    #[test]
    fn valid_git_cache_json_is_loaded() {
        let _lock = env_lock().lock().expect("lock");
        let _guard = EnvGuard::new(&["LYNX_CACHE_GIT_STATE", "PWD"]);
        std::env::set_var("LYNX_CACHE_GIT_STATE", r#"{"branch":"main","dirty":"0"}"#);
        std::env::set_var("PWD", "/");
        let ctx = build_render_context_from_env();
        assert_eq!(ctx.cache["git_state"]["branch"], "main");
    }

    #[test]
    fn invalid_git_cache_json_is_ignored() {
        let _lock = env_lock().lock().expect("lock");
        let _guard = EnvGuard::new(&["LYNX_CACHE_GIT_STATE", "PWD"]);
        std::env::set_var("LYNX_CACHE_GIT_STATE", "not-json");
        std::env::set_var("PWD", "/");
        let ctx = build_render_context_from_env();
        assert!(!ctx.cache.contains_key("git_state"));
    }

    #[test]
    fn valid_kubectl_cache_json_is_loaded() {
        let _lock = env_lock().lock().expect("lock");
        let _guard = EnvGuard::new(&["LYNX_CACHE_KUBECTL_STATE", "PWD"]);
        std::env::set_var(
            "LYNX_CACHE_KUBECTL_STATE",
            r#"{"context":"dev","namespace":"default"}"#,
        );
        std::env::set_var("PWD", "/");
        let ctx = build_render_context_from_env();
        assert_eq!(ctx.cache["kubectl_state"]["context"], "dev");
    }

    #[test]
    fn profile_state_is_loaded_from_config() {
        let _lock = env_lock().lock().expect("lock");
        let _guard = EnvGuard::new(&["HOME", "PWD", "LYNX_DIR"]);
        let home = temp_home();
        std::env::set_var("HOME", home.path());
        std::env::remove_var("LYNX_DIR");
        std::env::set_var("PWD", "/");
        let config_dir = home.path().join(lynx_core::brand::CONFIG_DIR);
        std::fs::create_dir_all(&config_dir).expect("create config dir");
        std::fs::write(
            config_dir.join("config.toml"),
            r#"schema_version = 1
enabled_plugins = []
active_theme = "default"
active_context = "interactive"
active_profile = "work"
"#,
        )
        .expect("write config");

        let ctx = build_render_context_from_env();
        assert_eq!(ctx.cache["profile_state"]["name"], "work");
    }
}
