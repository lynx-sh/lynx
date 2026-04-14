use anyhow::Result;
use clap::{Args, Subcommand};
use lynx_config::schema::UserPath;
use lynx_shell::path::{add_path, list_paths, remove_path, ResolvedPath};
use lynx_tui::{ListItem, TuiColors};

#[derive(Args)]
pub struct PathArgs {
    #[command(subcommand)]
    pub command: Option<PathCommand>,
}

#[derive(Subcommand)]
pub enum PathCommand {
    /// List all user-managed PATH entries
    List,
    /// Add a path entry (takes effect on next shell start)
    Add {
        /// The filesystem path to add to PATH
        path: String,
        /// Optional human-readable label (e.g. "Homebrew sbin")
        #[arg(short, long)]
        label: Option<String>,
    },
    /// Remove a path entry
    Remove {
        /// The path string to remove
        path: String,
    },
}

pub fn run(args: PathArgs) -> Result<()> {
    match args.command.unwrap_or(PathCommand::List) {
        PathCommand::List => cmd_list(),
        PathCommand::Add { path, label } => cmd_add(path, label),
        PathCommand::Remove { path } => cmd_remove(&path),
    }
}

// ── TUI wrapper ────────────────────────────────────────────────────────────

/// Display wrapper so ResolvedPath satisfies the ListItem trait.
struct PathRow(ResolvedPath);

impl ListItem for PathRow {
    fn title(&self) -> &str {
        &self.0.path
    }

    fn subtitle(&self) -> String {
        self.0.label.clone().unwrap_or_default()
    }

    fn detail(&self) -> String {
        let label = self.0.label.as_deref().unwrap_or("-");
        format!("path:  {}\nlabel: {label}", self.0.path)
    }

    fn category(&self) -> Option<&str> {
        Some("user")
    }

    fn is_active(&self) -> bool {
        true
    }
}

// ── Command handlers ───────────────────────────────────────────────────────

fn cmd_list() -> Result<()> {
    let cfg = lynx_config::load()?;
    let paths = list_paths(&cfg);

    if paths.is_empty() {
        println!("No managed paths. Use `lx path add <path>` to add one.");
        return Ok(());
    }

    let rows: Vec<PathRow> = paths.into_iter().map(PathRow).collect();
    let colors = TuiColors::default();
    // show() calls gate::tui_enabled() internally — no custom TTY check needed.
    lynx_tui::show(&rows, "Managed Paths", &colors).ok();
    Ok(())
}

fn cmd_add(path: String, label: Option<String>) -> Result<()> {
    let entry = UserPath {
        path: path.clone(),
        label,
    };
    add_path(entry)?;
    println!("path '{}' added — takes effect on next shell start", path);
    Ok(())
}

fn cmd_remove(path: &str) -> Result<()> {
    remove_path(path)?;
    println!("path '{}' removed — takes effect on next shell start", path);
    Ok(())
}
