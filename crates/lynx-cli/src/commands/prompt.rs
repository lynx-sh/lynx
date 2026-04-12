use anyhow::Result;
use clap::{Args, Subcommand};
use lynx_config::load as load_config;
use lynx_core::{brand, env_vars, types::Context};
use lynx_prompt::{
    cache_keys,
    evaluator::evaluate_theme,
    renderer::{render_prompt, render_transient_prompt},
    segment::RenderContext,
    AwsProfileSegment, BackgroundJobsSegment, CmdDurationSegment, CondaEnvSegment, ContextBadgeSegment, DirSegment,
    ExitCodeSegment, GitActionSegment, GitAheadBehindSegment, GitBranchSegment, GitShaSegment,
    GitStashSegment, GitStatusSegment, GitTimeSinceCommitSegment, GolangVersionSegment,
    HistNumberSegment, HostnameSegment, KubectlContextSegment, NewlineSegment,
    NodeVersionSegment, ProfileBadgeSegment, PromptCharSegment, RubyVersionSegment,
    RustVersionSegment, SshIndicatorSegment, TaskStatusSegment, TimeSegment, UsernameSegment,
    VenvSegment, ViModeSegment, CustomSegment,
};
use lynx_theme::loader::load as load_theme;
use std::collections::HashMap;

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub struct PromptArgs {
    #[command(subcommand)]
    pub command: PromptCommand,
}

#[derive(Subcommand)]
pub enum PromptCommand {
    /// Render PROMPT and RPROMPT shell assignments for eval by precmd hook
    Render {
        /// Emit a minimal transient PROMPT (collapses full prompt after a command runs).
        #[arg(long)]
        transient: bool,
    },
}

pub async fn run(args: PromptArgs) -> Result<()> {
    match args.command {
        PromptCommand::Render { transient } => cmd_render(transient).await,
    }
}

async fn cmd_render(transient: bool) -> Result<()> {
    let ctx = build_render_context_from_env();

    // Run in-process plugin lifecycle and emit shell:precmd so plugin handlers
    // fire before the prompt is built. Bus is discarded when this fn returns.
    let plugins_dir = lynx_core::paths::installed_plugins_dir();
    let bus = crate::bus::build_active_bus(&ctx.shell_context, &plugins_dir);
    bus.emit(lynx_events::types::Event::new(
        lynx_events::types::SHELL_PRECMD,
        &ctx.cwd,
    ))
    .await;

    // --- Load theme ---
    // Priority: LYNX_THEME env var (runtime override) → config.active_theme
    // (user's configured choice) → brand::DEFAULT_THEME (last-resort fallback).
    let config_theme = load_config()
        .map(|c| c.active_theme)
        .unwrap_or_else(|_| brand::DEFAULT_THEME.into());
    let theme_name = std::env::var(env_vars::LYNX_THEME).unwrap_or(config_theme);
    let theme = match load_theme(&theme_name) {
        Ok(t) => t,
        Err(e) => {
            println!(
                "lx: theme '{}' failed to load ({e}); falling back to '{}' — run `lx doctor` for details",
                theme_name,
                brand::DEFAULT_THEME
            );
            load_theme(brand::DEFAULT_THEME)?
        }
    };

    // --- Build segment registry ---
    let segments: Vec<Box<dyn lynx_prompt::segment::Segment>> = vec![
        Box::new(UsernameSegment),
        Box::new(HostnameSegment),
        Box::new(SshIndicatorSegment),
        Box::new(DirSegment),
        Box::new(GitBranchSegment),
        Box::new(GitStatusSegment),
        Box::new(GitActionSegment),
        Box::new(GitAheadBehindSegment),
        Box::new(GitShaSegment),
        Box::new(GitStashSegment),
        Box::new(GitTimeSinceCommitSegment),
        Box::new(AwsProfileSegment),
        Box::new(HistNumberSegment),
        Box::new(KubectlContextSegment),
        Box::new(NodeVersionSegment),
        Box::new(RubyVersionSegment),
        Box::new(GolangVersionSegment),
        Box::new(RustVersionSegment),
        Box::new(VenvSegment),
        Box::new(CondaEnvSegment),
        Box::new(ProfileBadgeSegment),
        Box::new(TaskStatusSegment),
        Box::new(CmdDurationSegment),
        Box::new(ExitCodeSegment),
        Box::new(BackgroundJobsSegment),
        Box::new(ViModeSegment),
        Box::new(CustomSegment),
        Box::new(TimeSegment),
        Box::new(ContextBadgeSegment),
        Box::new(NewlineSegment),
        Box::new(PromptCharSegment),
    ];

    // --- Evaluate and render ---
    let theme_ref = &theme;
    if transient {
        print!("{}", render_transient_prompt(theme_ref));
        return Ok(());
    }

    let columns = ctx.env.get("COLUMNS").and_then(|v| v.parse::<u32>().ok());
    let (left, right, top, top_right, continuation) = evaluate_theme(&segments, theme_ref, &ctx).await;
    let output = render_prompt(&left, &right, &top, &top_right, &continuation, theme_ref, columns);
    print!("{}", output);
    Ok(())
}

fn build_render_context_from_env() -> RenderContext {
    let cwd = std::env::var("PWD").unwrap_or_else(|_| "/".into());
    let shell_context = std::env::var(lynx_core::env_vars::LYNX_CONTEXT)
        .ok()
        .and_then(|v| Context::from_str(&v))
        .unwrap_or(Context::Interactive);
    let last_cmd_ms = std::env::var(env_vars::LYNX_LAST_CMD_MS)
        .ok()
        .and_then(|v| v.parse::<u64>().ok());

    let mut cache: HashMap<String, serde_json::Value> = HashMap::new();

    if let Ok(git_json) = std::env::var(env_vars::LYNX_CACHE_GIT_STATE) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&git_json) {
            cache.insert(cache_keys::GIT_STATE.into(), v);
        }
    }

    if let Ok(kubectl_json) = std::env::var(env_vars::LYNX_CACHE_KUBECTL_STATE) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&kubectl_json) {
            cache.insert(cache_keys::KUBECTL_STATE.into(), v);
        }
    }

    if let Ok(json) = std::env::var(env_vars::LYNX_CACHE_NODE_STATE) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&json) {
            cache.insert(cache_keys::NODE_STATE.into(), v);
        }
    }

    if let Ok(json) = std::env::var(env_vars::LYNX_CACHE_RUBY_STATE) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&json) {
            cache.insert(cache_keys::RUBY_STATE.into(), v);
        }
    }

    if let Ok(json) = std::env::var(env_vars::LYNX_CACHE_GOLANG_STATE) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&json) {
            cache.insert(cache_keys::GOLANG_STATE.into(), v);
        }
    }

    if let Ok(json) = std::env::var(env_vars::LYNX_CACHE_RUST_STATE) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&json) {
            cache.insert(cache_keys::RUST_STATE.into(), v);
        }
    }

    if let Ok(config) = load_config() {
        if let Some(profile) = &config.active_profile {
            if !profile.is_empty() {
                cache.insert(
                    cache_keys::PROFILE_STATE.into(),
                    serde_json::json!({ "name": profile }),
                );
            }
        }
    }

    // Capture env snapshot — segments must read from ctx.env, not std::env::var().
    let env_keys = [
        "USER",
        "UID",
        "HOSTNAME",
        "HOME",
        "SSH_CONNECTION",
        "SSH_TTY",
        "VIRTUAL_ENV",
        "CONDA_DEFAULT_ENV",
        env_vars::LYNX_LAST_EXIT_CODE,
        env_vars::LYNX_BG_JOBS,
        env_vars::LYNX_VI_MODE,
        "COLUMNS",
    ];
    let env: HashMap<String, String> = env_keys
        .iter()
        .filter_map(|k| std::env::var(k).ok().map(|v| (k.to_string(), v)))
        .collect();

    RenderContext {
        cwd,
        shell_context,
        last_cmd_ms,
        cache,
        env,
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
        let _guard = EnvGuard::new(&[lynx_core::env_vars::LYNX_LAST_CMD_MS, "PWD"]);
        std::env::set_var(lynx_core::env_vars::LYNX_LAST_CMD_MS, "1234");
        std::env::set_var("PWD", "/");
        let ctx = build_render_context_from_env();
        assert_eq!(ctx.last_cmd_ms, Some(1234));
    }

    #[test]
    fn invalid_last_command_ms_is_ignored() {
        let _lock = env_lock().lock().expect("lock");
        let _guard = EnvGuard::new(&[lynx_core::env_vars::LYNX_LAST_CMD_MS, "PWD"]);
        std::env::set_var(lynx_core::env_vars::LYNX_LAST_CMD_MS, "not-a-number");
        std::env::set_var("PWD", "/");
        let ctx = build_render_context_from_env();
        assert_eq!(ctx.last_cmd_ms, None);
    }

    #[test]
    fn valid_git_cache_json_is_loaded() {
        let _lock = env_lock().lock().expect("lock");
        let _guard = EnvGuard::new(&[lynx_core::env_vars::LYNX_CACHE_GIT_STATE, "PWD"]);
        std::env::set_var(lynx_core::env_vars::LYNX_CACHE_GIT_STATE, r#"{"branch":"main","dirty":false,"staged":false,"modified":false,"untracked":false,"stash":0,"ahead":0,"behind":0}"#);
        std::env::set_var("PWD", "/");
        let ctx = build_render_context_from_env();
        assert_eq!(ctx.cache[lynx_prompt::cache_keys::GIT_STATE]["branch"], "main");
        assert_eq!(ctx.cache[lynx_prompt::cache_keys::GIT_STATE]["staged"], false);
    }

    #[test]
    fn invalid_git_cache_json_is_ignored() {
        let _lock = env_lock().lock().expect("lock");
        let _guard = EnvGuard::new(&[lynx_core::env_vars::LYNX_CACHE_GIT_STATE, "PWD"]);
        std::env::set_var(lynx_core::env_vars::LYNX_CACHE_GIT_STATE, "not-json");
        std::env::set_var("PWD", "/");
        let ctx = build_render_context_from_env();
        assert!(!ctx.cache.contains_key(lynx_prompt::cache_keys::GIT_STATE));
    }

    #[test]
    fn valid_kubectl_cache_json_is_loaded() {
        let _lock = env_lock().lock().expect("lock");
        let _guard = EnvGuard::new(&[lynx_core::env_vars::LYNX_CACHE_KUBECTL_STATE, "PWD"]);
        std::env::set_var(
            lynx_core::env_vars::LYNX_CACHE_KUBECTL_STATE,
            r#"{"context":"dev","namespace":"default"}"#,
        );
        std::env::set_var("PWD", "/");
        let ctx = build_render_context_from_env();
        assert_eq!(ctx.cache[lynx_prompt::cache_keys::KUBECTL_STATE]["context"], "dev");
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
            config_dir.join(lynx_core::brand::CONFIG_FILE),
            r#"schema_version = 1
enabled_plugins = []
active_theme = "default"
active_context = "interactive"
active_profile = "work"
"#,
        )
        .expect("write config");

        let ctx = build_render_context_from_env();
        assert_eq!(ctx.cache[lynx_prompt::cache_keys::PROFILE_STATE]["name"], "work");
    }

    #[test]
    fn env_snapshot_captures_lynx_exit_code() {
        let _lock = env_lock().lock().expect("lock");
        let _guard = EnvGuard::new(&[lynx_core::env_vars::LYNX_LAST_EXIT_CODE, "PWD"]);
        std::env::set_var(lynx_core::env_vars::LYNX_LAST_EXIT_CODE, "1");
        std::env::set_var("PWD", "/");
        let ctx = build_render_context_from_env();
        assert_eq!(ctx.env.get("LYNX_LAST_EXIT_CODE").map(|s| s.as_str()), Some("1"));
    }

    #[test]
    fn env_snapshot_captures_virtual_env() {
        let _lock = env_lock().lock().expect("lock");
        let _guard = EnvGuard::new(&["VIRTUAL_ENV", "PWD"]);
        std::env::set_var("VIRTUAL_ENV", "/home/user/.venv/myproject");
        std::env::set_var("PWD", "/");
        let ctx = build_render_context_from_env();
        assert_eq!(
            ctx.env.get("VIRTUAL_ENV").map(|s| s.as_str()),
            Some("/home/user/.venv/myproject")
        );
    }

    #[test]
    fn env_snapshot_omits_unset_vars() {
        let _lock = env_lock().lock().expect("lock");
        let _guard = EnvGuard::new(&["SSH_CONNECTION", "PWD"]);
        std::env::remove_var("SSH_CONNECTION");
        std::env::set_var("PWD", "/");
        let ctx = build_render_context_from_env();
        assert!(!ctx.env.contains_key("SSH_CONNECTION"));
    }
}
