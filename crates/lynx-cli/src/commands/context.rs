use anyhow::Result;
use clap::{Args, Subcommand};

use lynx_config::{load, save};
use lynx_core::types::Context;
use lynx_events::{emit_event, types::Event};

#[derive(Args)]
pub struct ContextArgs {
    #[command(subcommand)]
    pub command: ContextCommand,
}

#[derive(Subcommand)]
pub enum ContextCommand {
    /// Switch to a context (interactive, agent, minimal)
    Set { name: String },
    /// Show current context and detection method
    Status,
}

pub async fn run(args: ContextArgs) -> Result<()> {
    match args.command {
        ContextCommand::Set { name } => cmd_set(&name),
        ContextCommand::Status => cmd_status(),
    }
}

fn cmd_set(name: &str) -> Result<()> {
    let ctx = parse_context(name)?;
    let mut cfg = load()?;
    cfg.active_context = ctx.clone();
    save(&cfg)?;

    // Emit shell:context-changed so the loader reloads plugins in-place.
    let data = serde_json::json!({ "context": name }).to_string();
    let event = Event::new("shell:context-changed", data);
    let _ = emit_event(&event);

    // Print the eval-bridge export statement for the shell to evaluate.
    println!("export LYNX_CONTEXT={name}");
    eprintln!("context switched to '{name}'");
    Ok(())
}

fn cmd_status() -> Result<()> {
    let cfg = load()?;

    // Determine detection method.
    let env_override = std::env::var("LYNX_CONTEXT").ok();
    let (detected, method) = if let Some(ref v) = env_override {
        (v.as_str().to_string(), "manual (LYNX_CONTEXT env var)")
    } else {
        let auto = detect_context_auto();
        (format!("{auto:?}").to_lowercase(), "auto-detected")
    };

    println!("Context:   {}", context_str(&cfg.active_context));
    println!("Detected:  {detected} ({method})");
    println!("Stored:    {}", context_str(&cfg.active_context));

    Ok(())
}

fn detect_context_auto() -> Context {
    // Mirror the detection logic from lynx-shell.
    if std::env::var("CLAUDE_CODE_ENTRYPOINT").is_ok()
        || std::env::var("CURSOR_SESSION_ID").is_ok()
    {
        Context::Agent
    } else if std::env::var("CI").is_ok() {
        Context::Minimal
    } else {
        Context::Interactive
    }
}

fn context_str(ctx: &Context) -> &'static str {
    match ctx {
        Context::Interactive => "interactive",
        Context::Agent => "agent",
        Context::Minimal => "minimal",
    }
}

fn parse_context(s: &str) -> anyhow::Result<Context> {
    match s {
        "interactive" => Ok(Context::Interactive),
        "agent" => Ok(Context::Agent),
        "minimal" => Ok(Context::Minimal),
        other => anyhow::bail!("unknown context '{}' — valid: interactive, agent, minimal", other),
    }
}
