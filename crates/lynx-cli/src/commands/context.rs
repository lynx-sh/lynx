use anyhow::Result;
use clap::{Args, Subcommand};

use lynx_config::{load, snapshot::mutate_config_transaction};
use lynx_core::types::Context;
use lynx_events::types::{Event, SHELL_CONTEXT_CHANGED};
use lynx_shell::context::{detect_context_outcome, DetectionMethod};

#[derive(Args)]
pub struct ContextArgs {
    #[command(subcommand)]
    pub command: Option<ContextCommand>,
}

#[derive(Subcommand)]
pub enum ContextCommand {
    /// Switch to a context (interactive, agent, minimal)
    Set { name: String },
    /// Show current context and detection method (default when no subcommand given)
    Status,
}

pub async fn run(args: ContextArgs) -> Result<()> {
    match args.command.unwrap_or(ContextCommand::Status) {
        ContextCommand::Set { name } => cmd_set(&name).await,
        ContextCommand::Status => cmd_status(),
    }
}

async fn cmd_set(name: &str) -> Result<()> {
    let ctx = parse_context(name)?;
    mutate_config_transaction("context-set", |cfg| {
        cfg.active_context = ctx.clone();
        Ok(())
    })?;

    // Emit shell:context-changed in-process so plugin handlers fire.
    let plugins_dir = lynx_core::paths::installed_plugins_dir();
    let bus = crate::bus::build_active_bus(&ctx, &plugins_dir);
    let data = serde_json::json!({ "context": name }).to_string();
    bus.emit(Event::new(SHELL_CONTEXT_CHANGED, data)).await;

    // Print the eval-bridge export statement for the shell to evaluate.
    println!("export LYNX_CONTEXT={name}");
    eprintln!("context switched to '{name}'");
    Ok(())
}

fn cmd_status() -> Result<()> {
    let cfg = load()?;
    let outcome = detect_context_outcome();
    let detected = context_str(&outcome.context).to_string();
    let method = match outcome.method {
        DetectionMethod::Override => "manual override (LYNX_CONTEXT)".to_string(),
        DetectionMethod::AgentEnv(var) => format!("auto-detected agent ({var})"),
        DetectionMethod::MinimalEnv(var) => format!("auto-detected minimal ({var})"),
        DetectionMethod::DefaultInteractive => "auto-detected interactive (default)".to_string(),
    };

    println!("Context:   {}", context_str(&cfg.active_context));
    println!("Detected:  {detected} ({method})");
    println!("Stored:    {}", context_str(&cfg.active_context));

    Ok(())
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
        other => anyhow::bail!(
            "unknown context '{}' — valid: interactive, agent, minimal",
            other
        ),
    }
}
