use anyhow::{Result};
use lynx_core::error::LynxError;
use clap::Args;

#[derive(Args)]
pub struct ExamplesArgs {
    /// Show examples for a specific command (plugin, theme, cron, run, jobs, dashboard, event, config, doctor, context, daemon)
    pub command: Option<String>,
}

pub async fn run(args: ExamplesArgs) -> Result<()> {
    match args.command.as_deref() {
        Some("plugin") => print_plugin_examples(),
        Some("theme") => print_theme_examples(),
        Some("cron") | Some("task") => print_task_examples(),
        Some("event") => print_event_examples(),
        Some("config") => print_config_examples(),
        Some("doctor") => print_doctor_examples(),
        Some("context") => print_context_examples(),
        Some("daemon") => print_daemon_examples(),
        Some("run") | Some("workflow") => print_workflow_examples(),
        Some("jobs") => print_jobs_examples(),
        Some("dashboard") => print_dashboard_examples(),
        Some(other) => {
            return Err(LynxError::unknown_command(other, "examples").into());
        }
        None => print_quickstart(),
    }
    Ok(())
}

fn print_quickstart() {
    println!(
        r#"
  Lynx — quickstart examples
  ───────────────────────────

  # First-time setup
  lx doctor                        # check your installation
  lx theme list                    # see available themes
  lx theme set nord                # switch theme
  lx theme random                  # try a random theme

  # Plugins
  lx plugin search                 # browse registry plugins
  lx plugin add git-extras         # install from registry by name
  lx plugin add ./plugins/my-tools # install from local path
  lx plugin new my-tools           # scaffold a new plugin
  lx plugin list                   # see what's loaded

  # Cron (scheduled commands)
  lx cron add cleanup              # add a task (opens editor)
  lx cron list                     # see all tasks
  lx cron run cleanup              # run a task now

  # Configuration
  lx config show                   # view current config
  lx config validate               # check for errors
  lx context set agent             # switch context manually

  # Diagnostics
  lx doctor                        # full health check
  lx benchmark                     # see startup timing
  lx rollback                      # restore a config snapshot

  # Workflows
  lx run list                      # see available workflows
  lx run deploy env=staging        # execute with params
  lx run deploy --dry-run          # preview without running
  lx jobs list                     # view job history

  # Dashboard (web UI)
  lx dashboard                     # opens in browser

  # Per-command examples
  lx examples plugin               # plugin workflow
  lx examples theme                # theme workflow
  lx examples cron                 # cron scheduler
  lx examples run                  # workflow execution
  lx examples jobs                 # job management
  lx examples dashboard            # web dashboard
"#
    );
}

fn print_plugin_examples() {
    println!(
        r#"
  lx plugin — examples
  ─────────────────────

  # Install a plugin from the registry
  lx plugin search git             # find registry plugins
  lx plugin add git-extras         # install by name
  lx plugin list

  # Create and install a local plugin
  lx plugin new git-extras
  lx plugin add ./git-extras
  lx plugin list

  # Check what a plugin exports before adding
  cat ./git-extras/plugin.toml

  # Reload a plugin after editing it
  lx plugin reinstall git-extras

  # Remove a plugin
  lx plugin remove git-extras

  # See what shell code a plugin generates
  lx plugin exec git-extras
"#
    );
}

fn print_theme_examples() {
    println!(
        r#"
  lx theme — examples
  ────────────────────

  # Browse and switch themes
  lx theme list
  lx theme set nord
  lx theme set default

  # Try a random theme (good for exploring)
  lx theme random

  # Edit the current theme
  lx theme edit

  # Reset to default if something looks broken
  lx theme set default && lx doctor
"#
    );
}

fn print_task_examples() {
    println!(
        r#"
  lx cron — examples
  ───────────────────

  # Schedule a daily backup (add task to tasks.toml)
  lx cron add backup

  # List all scheduled tasks and their last run status
  lx cron list

  # Run a task immediately (outside its schedule)
  lx cron run backup

  # Pause a task without removing it
  lx cron pause backup

  # Resume a paused task
  lx cron resume backup

  # View recent logs for a task
  lx cron logs backup

  # Remove a task
  lx cron remove backup

  # tasks.toml format example:
  #   [[task]]
  #   name    = "backup"
  #   run     = "rsync -a ~/docs /media/backup/"
  #   cron    = "0 2 * * *"       # 2am daily
  #   timeout = "5m"
  #   on_fail = "log"
"#
    );
}

fn print_event_examples() {
    println!(
        r#"
  lx event — examples
  ────────────────────

  # Emit a custom event from a shell script
  lx event emit "project:opened" --data "$PWD"

  # Inspect recent events (useful for debugging)
  lx event log

  # Fire a hook when changing directories
  # (add to your plugin's plugin.toml)
  #   [load]
  #   hooks = ["chpwd"]
"#
    );
}

fn print_config_examples() {
    println!(
        r#"
  lx config — examples
  ─────────────────────

  # View the full current config
  lx config show

  # Open config in your $EDITOR
  lx config edit

  # Validate config without applying changes
  lx config validate

  # After manual edits, check for errors
  lx config validate && echo "Config is valid"

  # See config file location
  lx config show | head -1
"#
    );
}

fn print_doctor_examples() {
    println!(
        r#"
  lx doctor — examples
  ─────────────────────

  # Full health check (run this first if something seems off)
  lx doctor

  # Doctor checks:
  #   binary reachable at ~/.local/bin/lx
  #   config.toml parses without error
  #   active theme exists
  #   shell integration sourced in .zshrc
  #   plugin manifests all valid
  #   required binaries for each plugin present

  # Fix a specific issue, then re-run doctor
  lx theme set default
  lx doctor
"#
    );
}

fn print_context_examples() {
    println!(
        r#"
  lx context — examples
  ──────────────────────

  # See current context
  lx context status

  # Switch to agent context (disables aliases, minimal prompt)
  lx context set agent

  # Switch back to interactive
  lx context set interactive

  # Contexts are auto-detected — CLAUDECODE or CURSOR_CLI env vars
  # trigger agent context automatically. Manual override is for testing.
"#
    );
}

fn print_daemon_examples() {
    println!(
        r#"
  lx daemon — examples
  ─────────────────────

  # Start the Lynx background daemon (runs task scheduler)
  lx daemon start

  # Check daemon status
  lx daemon status

  # Stop the daemon
  lx daemon stop

  # Register as a system service (launchd on macOS, systemd on Linux)
  lx daemon install

  # Restart daemon to pick up config changes (after editing tasks.toml)
  lx daemon restart
"#
    );
}

pub fn print_workflow_examples() {
    println!(
        r#"
  lx run — workflow examples
  ──────────────────────────

  Workflows are saved recipes — a list of commands you run together.
  They live as .toml files in ~/.config/lynx/workflows/

  ── Example 1: Simple — run two commands in sequence ──────────

  File: ~/.config/lynx/workflows/check.toml

    [workflow]
    name = "check"
    description = "Lint and test my project"

    [[step]]
    name = "lint"
    run = "cargo clippy --all"

    [[step]]
    name = "test"
    run = "cargo nextest run --all"

  Run it:  lx run check

  ── Example 2: Parallel steps — run lint + test at the same time ──

    [[step]]
    name = "lint"
    run = "cargo clippy --all"
    group = "checks"              # <-- same group = parallel

    [[step]]
    name = "test"
    run = "cargo nextest run"
    group = "checks"              # <-- same group = parallel

    [[step]]
    name = "build"
    run = "cargo build --release"
    depends_on = ["lint", "test"] # <-- waits for both to finish

  ── Example 3: Parameters — make a workflow reusable ─────────

    [workflow]
    name = "deploy"
    description = "Deploy to an environment"

    [[workflow.param]]
    name = "env"
    type = "string"
    choices = ["staging", "production"]

    [[workflow.param]]
    name = "skip_tests"
    type = "bool"
    default = "false"

    [[step]]
    name = "build"
    run = "cargo build --release"

    [[step]]
    name = "push"
    runner = "bash"
    run = "./deploy.sh $env"
    confirm = true                # <-- asks "are you sure?" first

  Run it:  lx run deploy env=staging
           lx run deploy env=production skip_tests=true

  ── Example 4: Error handling and timeouts ───────────────────

    [[step]]
    name = "migrate"
    run = "diesel migration run"
    timeout_sec = 120             # kill if it takes > 2 minutes
    on_fail = "abort"             # stop the whole workflow (default)

    [[step]]
    name = "seed"
    run = "cargo run --bin seed"
    on_fail = "continue"          # keep going even if this fails

    [[step]]
    name = "health-check"
    run = "curl -f http://localhost:8080/health"
    on_fail = "retry"             # try again on failure
    retry_count = 3               # up to 3 times

  ── Useful commands ──────────────────────────────────────────

  lx run list                     browse available workflows
  lx run deploy --dry-run         see what would run (no execution)
  lx run deploy --bg              run in background
  lx run deploy --yes             skip confirmation prompts
  lx jobs list                    see running/finished jobs
"#
    );
}

fn print_jobs_examples() {
    println!(
        r#"
  lx jobs — examples
  ───────────────────

  # List recent jobs
  lx jobs list

  # View full details for a job
  lx jobs view deploy-20260412-143025

  # View job log output
  lx jobs log deploy-20260412-143025

  # Kill a running job
  lx jobs kill deploy-20260412-143025

  # Clean old job records (default: older than 72 hours)
  lx jobs clean
  lx jobs clean --hours 24
"#
    );
}

fn print_dashboard_examples() {
    println!(
        r#"
  lx dashboard — examples
  ────────────────────────

  # Open the dashboard (starts local server, opens browser)
  lx dashboard

  # Dashboard provides a full web UI for:
  #   - Overview: system health, active theme, plugin count
  #   - Themes: WYSIWYG editor with live prompt preview
  #   - Plugins: enable/disable toggles, install from registry
  #   - Registry: browse packages by type, manage taps
  #   - Workflows: view available workflows
  #   - Cron: manage scheduled tasks
  #   - Intros: preview and switch startup intros
  #   - System: config editor, doctor diagnostics, diag log
  #
  # Press Ctrl-C to stop the dashboard server.
"#
    );
}
