use anyhow::Result;
use clap::{Args, Subcommand};
use lynx_config::load as load_config;
use lynx_core::types::Context;
use lynx_prompt::{
    evaluator::evaluate_theme,
    renderer::render_prompt,
    segment::RenderContext,
    CmdDurationSegment, ContextBadgeSegment, DirSegment, GitBranchSegment, GitStatusSegment,
    KubectlContextSegment, ProfileBadgeSegment, TaskStatusSegment,
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
    // --- Build RenderContext from environment ---

    let cwd = std::env::var("PWD").unwrap_or_else(|_| "/".into());

    let shell_context = match std::env::var("LYNX_CONTEXT").as_deref() {
        Ok("agent") => Context::Agent,
        Ok("minimal") => Context::Minimal,
        _ => Context::Interactive,
    };

    let last_cmd_ms = std::env::var("LYNX_LAST_CMD_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok());

    // --- Populate cache from env vars written by the precmd hook ---
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

    // profile_state comes from config (no shell-side cache needed)
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

    let ctx = RenderContext { cwd, shell_context, last_cmd_ms, cache };

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
