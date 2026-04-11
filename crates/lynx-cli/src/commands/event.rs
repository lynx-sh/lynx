use anyhow::Result;
use clap::{Args, Subcommand};
use lynx_events::{
    bridge::{emit_event, register_subscriber},
    logger,
    types::Event,
};

#[derive(Args)]
pub struct EventArgs {
    #[command(subcommand)]
    pub command: EventCommand,
}

#[derive(Subcommand)]
pub enum EventCommand {
    /// Emit a named event to the daemon (fire-and-forget)
    Emit {
        /// Event name, e.g. shell:chpwd
        name: String,
        /// Optional data payload
        #[arg(long, default_value = "")]
        data: String,
    },
    /// Register a zsh function as a subscriber for an event
    On {
        /// Event name to subscribe to
        event_name: String,
        /// Zsh function name to call when the event fires
        zsh_fn: String,
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
}

pub async fn run(args: EventArgs) -> Result<()> {
    match args.command {
        EventCommand::Emit { name, data } => {
            emit_event(&Event::new(name, data))?;
        }
        EventCommand::On { event_name, zsh_fn } => {
            register_subscriber(&event_name, &zsh_fn)?;
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
            })
            .await?;
        }
    }
    Ok(())
}
