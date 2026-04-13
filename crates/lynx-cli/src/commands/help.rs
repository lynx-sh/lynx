//! Interactive help browser for bare `lx` command.

use anyhow::Result;

/// A help topic entry for the interactive list.
struct HelpEntry {
    command: &'static str,
    description: &'static str,
    usage: &'static str,
    category: &'static str,
    /// Extended detail shown when the entry is selected. Empty = default.
    extra: &'static str,
}

impl lynx_tui::ListItem for HelpEntry {
    fn title(&self) -> &str {
        self.command
    }

    fn subtitle(&self) -> String {
        self.description.to_string()
    }

    fn detail(&self) -> String {
        let mut lines = vec![
            self.description.to_string(),
            String::new(),
            format!("Usage: lx {}", self.usage),
        ];
        if !self.category.is_empty() {
            lines.push(String::new());
            lines.push(format!("Category: {}", self.category));
        }
        if !self.extra.is_empty() {
            lines.push(String::new());
            lines.push(self.extra.to_string());
        }
        lines.join("\n")
    }

    fn category(&self) -> Option<&str> {
        Some(self.category)
    }

    fn tags(&self) -> Vec<&str> {
        vec![self.category]
    }
}

/// All commands organized by category.
const ENTRIES: &[HelpEntry] = &[
    // ── Core ──
    HelpEntry {
        command: "setup",
        description: "Set up Lynx files and optionally patch .zshrc",
        usage: "setup [--zshrc]",
        category: "core",
        extra: "",
    },
    HelpEntry {
        command: "doctor",
        description: "Diagnose issues with your Lynx setup",
        usage: "doctor",
        category: "core",
        extra: "",
    },
    HelpEntry {
        command: "update",
        description: "Check for and install lx updates",
        usage: "update",
        category: "core",
        extra: "",
    },
    HelpEntry {
        command: "uninstall",
        description: "Remove Lynx from this system",
        usage: "uninstall",
        category: "core",
        extra: "",
    },
    HelpEntry {
        command: "benchmark",
        description: "Benchmark startup time per component",
        usage: "benchmark",
        category: "core",
        extra: "",
    },
    // ── Themes ──
    HelpEntry {
        command: "theme list",
        description: "Browse and switch themes interactively",
        usage: "theme list",
        category: "themes",
        extra: "",
    },
    HelpEntry {
        command: "theme set",
        description: "Set the active theme by name",
        usage: "theme set <name>",
        category: "themes",
        extra: "",
    },
    HelpEntry {
        command: "theme edit",
        description: "Open active theme in VS Code for editing",
        usage: "theme edit",
        category: "themes",
        extra: "",
    },
    HelpEntry {
        command: "theme convert",
        description: "Convert OMZ or OMP themes to Lynx TOML",
        usage: "theme convert <source> [name] [--force]",
        category: "themes",
        extra: "",
    },
    HelpEntry {
        command: "theme patch",
        description: "Modify a single value in the active theme",
        usage: "theme patch <dot.path> <value>",
        category: "themes",
        extra: "",
    },
    // ── Plugins ──
    HelpEntry {
        command: "plugin list",
        description: "Browse installed plugins interactively",
        usage: "plugin list",
        category: "plugins",
        extra: "",
    },
    HelpEntry {
        command: "plugin add",
        description: "Add a plugin from a local path",
        usage: "plugin add <path>",
        category: "plugins",
        extra: "",
    },
    HelpEntry {
        command: "plugin enable",
        description: "Enable a disabled plugin",
        usage: "plugin enable <name>",
        category: "plugins",
        extra: "",
    },
    HelpEntry {
        command: "plugin disable",
        description: "Disable a plugin without removing it",
        usage: "plugin disable <name>",
        category: "plugins",
        extra: "",
    },
    // ── Registry ──
    HelpEntry {
        command: "install",
        description: "Install packages from the registry",
        usage: "install <name>",
        category: "registry",
        extra: "",
    },
    HelpEntry {
        command: "remove",
        description: "Remove a package's Lynx integration",
        usage: "remove <name>",
        category: "registry",
        extra: "",
    },
    HelpEntry {
        command: "browse",
        description: "Browse available packages by category",
        usage: "browse [category]",
        category: "registry",
        extra: "",
    },
    HelpEntry {
        command: "tap",
        description: "Manage package registry taps (sources)",
        usage: "tap <subcommand>",
        category: "registry",
        extra: "",
    },
    // ── Shell ──
    HelpEntry {
        command: "context",
        description: "Switch or show context (interactive, agent, minimal)",
        usage: "context [set <name>]",
        category: "shell",
        extra: "",
    },
    HelpEntry {
        command: "config",
        description: "Show, edit, or modify configuration",
        usage: "config <subcommand>",
        category: "shell",
        extra: "",
    },
    HelpEntry {
        command: "rollback",
        description: "Rollback config to a previous snapshot",
        usage: "rollback [--list]",
        category: "shell",
        extra: "",
    },
    HelpEntry {
        command: "intro",
        description: "Manage shell startup intros",
        usage: "intro <subcommand>",
        category: "shell",
        extra: "",
    },
    // ── Automation ──
    HelpEntry {
        command: "run",
        description: "Execute a workflow — reusable multi-step task recipes defined in TOML",
        usage: "run <workflow> [key=value ...] [--bg] [--dry-run] [--yes]",
        category: "automation",
        extra: "\
Workflows are reusable task recipes. Write a TOML file once, run it \
with one command. Lynx handles step ordering, parallelism, retries, \
and timeouts.

Getting started:
  1. Create a file in ~/.config/lynx/workflows/  (e.g. deploy.toml)
  2. Run it:  lx run deploy

Workflow file structure:
  [workflow]
  name = \"check\"
  description = \"Lint and test\"

  [[workflow.param]]          # optional typed parameters
  name = \"target\"
  type = \"string\"
  default = \"all\"

  [[step]]
  name = \"lint\"
  run = \"cargo clippy --all\"
  group = \"checks\"            # same group = run in parallel

  [[step]]
  name = \"test\"
  run = \"cargo nextest run --all\"
  group = \"checks\"

  [[step]]
  name = \"done\"
  run = \"echo All checks passed!\"
  depends_on = [\"lint\", \"test\"]  # waits for both

Step options:
  runner      sh (default), bash, zsh, python, node, go, cargo, curl, docker
  on_fail     abort (default), retry, continue
  retry_count number of retries (with on_fail = \"retry\")
  timeout_sec seconds before the step is killed
  confirm     true = prompt before running this step
  condition   \"$param == value\" — skip if false
  env         extra env vars for this step
  cwd         working directory for this step
  depends_on  steps that must finish first
  group       steps in the same group run in parallel

Flags:
  --dry-run   show what would run without executing
  --bg        run in background (check status with lx jobs)
  --yes       skip confirmation prompts

List workflows:  lx run list",
    },
    HelpEntry {
        command: "jobs",
        description: "View and manage workflow jobs",
        usage: "jobs [list|show|cancel] [id]",
        category: "automation",
        extra: "",
    },
    HelpEntry {
        command: "cron",
        description: "Manage scheduled cron tasks",
        usage: "cron <subcommand>",
        category: "automation",
        extra: "",
    },
    HelpEntry {
        command: "daemon",
        description: "Manage the Lynx background daemon",
        usage: "daemon [start|stop|status]",
        category: "automation",
        extra: "",
    },
    // ── Developer ──
    HelpEntry {
        command: "dev",
        description: "Developer utilities — sync, build, run",
        usage: "dev <subcommand>",
        category: "developer",
        extra: "",
    },
    HelpEntry {
        command: "diag",
        description: "View and manage the diagnostic log",
        usage: "diag [clear]",
        category: "developer",
        extra: "",
    },
    HelpEntry {
        command: "audit",
        description: "Audit plugins — exports, hooks, binary deps",
        usage: "audit [plugin-name]",
        category: "developer",
        extra: "",
    },
    HelpEntry {
        command: "examples",
        description: "Real-world usage examples and quickstart guide",
        usage: "examples [command]",
        category: "developer",
        extra: "",
    },
    HelpEntry {
        command: "dashboard",
        description: "Open the web-based management dashboard",
        usage: "dashboard [--port <port>]",
        category: "developer",
        extra: "",
    },
];

/// Show the interactive help browser. Returns Ok(()) always.
pub fn show_interactive_help() -> Result<()> {
    let tui_colors = load_tui_colors();

    if let Some(idx) = lynx_tui::show(ENTRIES, "Lynx Commands", &tui_colors)? {
        let entry = &ENTRIES[idx];
        // Print detailed help for the selected command.
        println!("\n  lx {}\n", entry.command);
        println!("  {}\n", entry.description);
        println!("  Usage: lx {}\n", entry.usage);
        println!("  For full options: lx {} --help", entry.command.split_whitespace().next().unwrap_or(entry.command));
    }

    Ok(())
}

/// Load TUI colors from active theme, with defaults.
fn load_tui_colors() -> lynx_tui::TuiColors {
    let Ok(cfg) = lynx_config::load() else {
        return lynx_tui::TuiColors::default();
    };
    match lynx_theme::loader::load(&cfg.active_theme) {
        Ok(theme) => lynx_tui::TuiColors::from_palette(&theme.colors),
        Err(_) => lynx_tui::TuiColors::default(),
    }
}
