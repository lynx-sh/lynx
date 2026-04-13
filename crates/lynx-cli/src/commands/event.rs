use anyhow::Result;
use clap::{Args, Subcommand};
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
            })
            .await?;
        }
        EventCommand::Other(args) => {
            anyhow::bail!("unknown event command '{}' — run `lx event` for help", args.first().map(|s| s.as_str()).unwrap_or(""))
        }
    }
    Ok(())
}
