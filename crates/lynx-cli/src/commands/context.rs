use anyhow::Result;
use clap::{Args, Subcommand};
use lynx_core::error::LynxError;

use lynx_config::{load, snapshot::mutate_config_transaction};
use lynx_core::env_vars;
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
                Err(LynxError::unknown_command(
                    super::unknown_subcmd_name(&args),
                    "context",
                )
                .into())
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
    let bus = crate::bus::build_active_bus();
    let data = serde_json::json!({ "context": name }).to_string();
    bus.emit(Event::new(SHELL_CONTEXT_CHANGED, data)).await;

    // Print the eval-bridge export statement for the shell to evaluate.
    println!("export {}={name}", env_vars::LYNX_CONTEXT);
    println!("context switched to '{name}'");
    Ok(())
}

fn cmd_status() -> Result<()> {
    let cfg = load()?;
    let outcome = detect_context_outcome();
    let detected = outcome.context.as_str().to_string();
    let method = match outcome.method {
        DetectionMethod::Override => format!("manual override ({})", env_vars::LYNX_CONTEXT),
        DetectionMethod::AgentEnv(var) => format!("auto-detected agent ({var})"),
        DetectionMethod::MinimalEnv(var) => format!("auto-detected minimal ({var})"),
        DetectionMethod::DefaultInteractive => "auto-detected interactive (default)".to_string(),
    };

    println!("Context:   {}", cfg.active_context.as_str());
    println!("Detected:  {detected} ({method})");
    println!("Stored:    {}", cfg.active_context.as_str());

    Ok(())
}

fn parse_context(s: &str) -> anyhow::Result<Context> {
    Context::parse(s).ok_or_else(|| {
        LynxError::NotFound {
            item_type: "Context".into(),
            name: s.to_string(),
            hint: "valid contexts: interactive, agent, minimal".into(),
        }
        .into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_context_interactive() {
        assert!(matches!(
            parse_context("interactive").unwrap(),
            Context::Interactive
        ));
    }

    #[test]
    fn parse_context_agent() {
        assert!(matches!(parse_context("agent").unwrap(), Context::Agent));
    }

    #[test]
    fn parse_context_minimal() {
        assert!(matches!(
            parse_context("minimal").unwrap(),
            Context::Minimal
        ));
    }

    #[test]
    fn parse_context_invalid_returns_not_found() {
        let err = parse_context("bogus").unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("bogus"),
            "error should contain the invalid name: {msg}"
        );
        assert!(
            msg.contains("Context"),
            "error should mention item type: {msg}"
        );
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
    fn context_as_str_round_trips() {
        assert_eq!(Context::Interactive.as_str(), "interactive");
        assert_eq!(Context::Agent.as_str(), "agent");
        assert_eq!(Context::Minimal.as_str(), "minimal");
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
