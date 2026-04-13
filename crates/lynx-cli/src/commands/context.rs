use lynx_core::error::LynxError;
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
    /// Smart dispatch: treat unknown subcommand as context name for `set`
    #[command(external_subcommand)]
    Other(Vec<String>),
}

pub async fn run(args: ContextArgs) -> Result<()> {
    match args.command.unwrap_or(ContextCommand::Status) {
        ContextCommand::Set { name } => cmd_set(&name).await,
        ContextCommand::Status => cmd_status(),
        ContextCommand::Other(args) => {
            if args.len() == 1 {
                cmd_set(&args[0]).await
            } else {
                Err(LynxError::unknown_command(args.first().map(|s| s.as_str()).unwrap_or(""), "context").into())
            }
        }
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
    println!("context switched to '{name}'");
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
        other => Err(LynxError::NotFound {
            item_type: "Context".into(),
            name: other.to_string(),
            hint: "valid contexts: interactive, agent, minimal".into(),
        }.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_context_interactive() {
        assert!(matches!(parse_context("interactive").unwrap(), Context::Interactive));
    }

    #[test]
    fn parse_context_agent() {
        assert!(matches!(parse_context("agent").unwrap(), Context::Agent));
    }

    #[test]
    fn parse_context_minimal() {
        assert!(matches!(parse_context("minimal").unwrap(), Context::Minimal));
    }

    #[test]
    fn parse_context_invalid_returns_not_found() {
        let err = parse_context("bogus").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("bogus"), "error should contain the invalid name: {msg}");
        assert!(msg.contains("Context"), "error should mention item type: {msg}");
    }

    #[test]
    fn parse_context_empty_string_is_error() {
        assert!(parse_context("").is_err());
    }

    #[test]
    fn parse_context_case_sensitive() {
        // "Interactive" (uppercase) should NOT match
        assert!(parse_context("Interactive").is_err());
        assert!(parse_context("AGENT").is_err());
    }

    #[test]
    fn context_str_round_trips() {
        assert_eq!(context_str(&Context::Interactive), "interactive");
        assert_eq!(context_str(&Context::Agent), "agent");
        assert_eq!(context_str(&Context::Minimal), "minimal");
    }

    #[test]
    fn default_command_is_status() {
        // When no subcommand given, should default to Status
        let cmd = ContextArgs { command: None };
        // The unwrap_or in run() converts None → Status
        let resolved = cmd.command.unwrap_or(ContextCommand::Status);
        assert!(matches!(resolved, ContextCommand::Status));
    }
}
