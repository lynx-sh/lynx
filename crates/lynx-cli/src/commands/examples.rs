use anyhow::Result;
use clap::Args;

#[derive(Args)]
pub struct ExamplesArgs {
    /// Show examples for a specific command (plugin, theme, task, event, config, doctor)
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
        Some(other) => {
            println!(
                "lx: unknown command '{other}' — try: plugin, theme, task, event, config, doctor"
            );
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
  lx plugin new my-tools           # scaffold a new plugin
  lx plugin add ./plugins/my-tools # install it
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

  # Per-command examples
  lx examples plugin               # plugin workflow
  lx examples theme                # theme workflow
  lx examples task                 # task scheduler workflow
"#
    );
}

fn print_plugin_examples() {
    println!(
        r#"
  lx plugin — examples
  ─────────────────────

  # Create and install a new plugin
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

  # Listen for events (blocks — useful for debugging)
  lx event on "shell:chpwd"

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

  # Reload daemon config without restart (after editing tasks.toml)
  lx daemon reload
"#
    );
}
