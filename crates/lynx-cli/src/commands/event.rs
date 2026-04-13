use anyhow::Result;
use clap::{Args, Subcommand};
use lynx_core::error::LynxError;
use lynx_events::{logger, types::Event};

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub struct EventArgs {
    #[command(subcommand)]
    pub command: EventCommand,
}

#[derive(Subcommand)]
pub enum EventCommand {
    /// Emit a named event in-process (runs registered plugin handlers, then exits)
    Emit {
        /// Event name, e.g. shell:chpwd
        name: String,
        /// Optional data payload
        #[arg(long, default_value = "")]
        data: String,
    },
    /// Inspect the event log
    Log {
        /// Number of recent entries to show
        #[arg(long, default_value = "20")]
        tail: usize,
        /// Filter by event name prefix (e.g. shell: or git:)
        #[arg(long)]
        filter: Option<String>,
    },
    /// Show real-world usage examples
    Examples,
    /// Catch unknown subcommands for friendly error
    #[command(external_subcommand)]
    Other(Vec<String>),
}

pub async fn run(args: EventArgs) -> Result<()> {
    match args.command {
        EventCommand::Emit { name, data } => {
            let config = lynx_config::load()?;
            let plugins_dir = lynx_core::paths::installed_plugins_dir();
            let bus = crate::bus::build_active_bus(&config.active_context, &plugins_dir);
            bus.emit(Event::new(name, data)).await;
        }
        EventCommand::Log { tail, filter } => {
            let entries = logger::tail_log(tail, filter.as_deref())?;
            if entries.is_empty() {
                println!("No events logged yet.");
            } else {
                for e in entries {
                    println!(
                        "[{}] {} | {} | {}",
                        e.timestamp, e.event_name, e.source, e.data
                    );
                }
            }
        }
        EventCommand::Examples => {
            crate::commands::examples::run(crate::commands::examples::ExamplesArgs {
                command: Some("event".into()),
            })?;
        }
        EventCommand::Other(args) => {
            return Err(LynxError::unknown_command(
                args.first().map(|s| s.as_str()).unwrap_or(""),
                "event",
            )
            .into())
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn event_unknown_subcommand_returns_error() {
        let args = EventArgs {
            command: EventCommand::Other(vec!["bogus".to_string()]),
        };
        let err = run(args).await.unwrap_err();
        assert!(err.to_string().contains("bogus"));
    }

    #[tokio::test]
    async fn event_unknown_subcommand_empty_returns_error() {
        let args = EventArgs {
            command: EventCommand::Other(vec![]),
        };
        // Should not panic even with empty args
        let result = run(args).await;
        assert!(result.is_err());
    }
}
