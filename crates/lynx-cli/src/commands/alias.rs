use anyhow::Result;
use clap::{Args, Subcommand};
use lynx_config::schema::{AliasContext, UserAlias};
use lynx_shell::alias::{add_alias, list_aliases, remove_alias, AliasSrc, ResolvedAlias};
use lynx_tui::{ListItem, TuiColors};

#[derive(Args)]
pub struct AliasArgs {
    #[command(subcommand)]
    pub command: Option<AliasCommand>,
}

#[derive(Subcommand)]
pub enum AliasCommand {
    /// List all active aliases (user-defined and plugin-provided)
    List,
    /// Add a user-defined alias
    Add {
        /// The alias name (e.g. gs)
        name: String,
        /// The command the alias expands to (e.g. "git status")
        command: String,
        /// Optional description
        #[arg(short, long)]
        description: Option<String>,
        /// Load in all contexts, not just interactive (default: interactive only)
        #[arg(long)]
        all_contexts: bool,
    },
    /// Remove a user-defined alias
    Remove {
        /// The alias name to remove
        name: String,
    },
}

pub fn run(args: AliasArgs) -> Result<()> {
    match args.command.unwrap_or(AliasCommand::List) {
        AliasCommand::List => cmd_list(),
        AliasCommand::Add {
            name,
            command,
            description,
            all_contexts,
        } => cmd_add(name, command, description, all_contexts),
        AliasCommand::Remove { name } => cmd_remove(&name),
    }
}

// ── TUI wrapper ────────────────────────────────────────────────────────────

/// Display wrapper so ResolvedAlias satisfies the ListItem trait.
struct AliasRow(ResolvedAlias);

impl ListItem for AliasRow {
    fn title(&self) -> &str {
        &self.0.name
    }

    fn subtitle(&self) -> String {
        self.0.command.clone()
    }

    fn detail(&self) -> String {
        let context = match self.0.context {
            AliasContext::Interactive => "interactive",
            AliasContext::All => "all",
        };
        let source = match &self.0.source {
            AliasSrc::User => "user".to_string(),
            AliasSrc::Plugin(name) => format!("plugin: {name}"),
        };
        let mut lines = vec![
            format!("name:    {}", self.0.name),
            format!("command: {}", self.0.command),
            format!("context: {context}"),
            format!("source:  {source}"),
        ];
        if let Some(desc) = &self.0.description {
            lines.push(format!("note:    {desc}"));
        }
        lines.join("\n")
    }

    fn category(&self) -> Option<&str> {
        Some(match &self.0.source {
            AliasSrc::User => "user",
            AliasSrc::Plugin(_) => "plugin",
        })
    }

    fn tags(&self) -> Vec<&str> {
        self.0
            .description
            .as_deref()
            .map(|d| vec![d])
            .unwrap_or_default()
    }

    fn is_active(&self) -> bool {
        matches!(self.0.source, AliasSrc::User)
    }
}

// ── Command handlers ───────────────────────────────────────────────────────

fn cmd_list() -> Result<()> {
    let cfg = lynx_config::load()?;
    let plugin_dir = lynx_core::paths::installed_plugins_dir();
    let aliases = list_aliases(&cfg, &plugin_dir);

    if aliases.is_empty() {
        println!("No aliases defined. Use `lx alias add <name> <command>` to add one.");
        return Ok(());
    }

    let rows: Vec<AliasRow> = aliases.into_iter().map(AliasRow).collect();
    let colors = TuiColors::default();
    // show() calls gate::tui_enabled() internally — no custom TTY check needed.
    lynx_tui::show(&rows, "Aliases", &colors).ok();
    Ok(())
}

fn cmd_add(
    name: String,
    command: String,
    description: Option<String>,
    all_contexts: bool,
) -> Result<()> {
    let context = if all_contexts {
        AliasContext::All
    } else {
        AliasContext::Interactive
    };
    let plugin_dir = lynx_core::paths::installed_plugins_dir();
    let alias = UserAlias {
        name: name.clone(),
        command: command.clone(),
        description,
        context,
    };
    add_alias(alias, &plugin_dir)?;
    // Eval-able output: the first line sets the alias live in the current shell,
    // the second line is user feedback. The shell wrapper evals both.
    println!("alias {}='{}'", name, command.replace('\'', "'\\''"));
    println!("alias '{}' added", name);
    Ok(())
}

fn cmd_remove(name: &str) -> Result<()> {
    remove_alias(name)?;
    // Plain confirmation — the shell wrapper runs `unalias "$3"` after this exits.
    println!("alias '{}' removed", name);
    Ok(())
}
