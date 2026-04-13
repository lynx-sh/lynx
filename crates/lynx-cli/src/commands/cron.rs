use anyhow::{Context, Result};
use lynx_core::error::LynxError;
use clap::{Args, Subcommand};
use lynx_task::{
    parse_tasks_file, read_last_run, read_tasks_file, write_tasks_file,
    schema::{OnFail, Task},
    validate_task,
};
use std::io::{BufRead, BufReader};

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub struct CronArgs {
    #[command(subcommand)]
    pub command: CronCommand,
}

#[derive(Subcommand)]
pub enum CronCommand {
    /// Add a scheduled task to tasks.toml
    Add {
        /// Task name (unique)
        name: String,
        /// Shell command to run
        #[arg(long)]
        run: String,
        /// Cron expression (5-field: min hr dom mon dow)
        #[arg(long)]
        cron: String,
        /// Description
        #[arg(long, default_value = "")]
        description: String,
        /// What to do on failure: log, notify, ignore
        #[arg(long, default_value = "log")]
        on_fail: String,
        /// Timeout (e.g. 60s, 5m, 1h)
        #[arg(long)]
        timeout: Option<String>,
        /// Write logs for this task
        #[arg(long, default_value_t = true)]
        log: bool,
    },
    /// List all scheduled tasks with status
    List,
    /// Show logs for a task
    Logs {
        /// Task name
        name: String,
        /// Number of recent lines to show
        #[arg(long, default_value_t = 20)]
        tail: usize,
        /// Stream new log entries as they arrive
        #[arg(long)]
        follow: bool,
    },
    /// Disable a task (set enabled=false)
    #[command(alias = "pause")]
    Disable {
        /// Task name
        name: String,
    },
    /// Enable a disabled task (set enabled=true)
    #[command(alias = "resume")]
    Enable {
        /// Task name
        name: String,
    },
    /// Run a task immediately (fires even if disabled)
    Run {
        /// Task name
        name: String,
    },
    /// Remove a task from tasks.toml
    Remove {
        /// Task name
        name: String,
    },
    /// Show real-world usage examples
    Examples,
    /// Catch unknown subcommands for friendly error
    #[command(external_subcommand)]
    Other(Vec<String>),
}

pub async fn run(args: CronArgs) -> Result<()> {
    match args.command {
        CronCommand::Add {
            name,
            run,
            cron,
            description,
            on_fail,
            timeout,
            log,
        } => cmd_add(name, run, cron, description, on_fail, timeout, log).await,
        CronCommand::List => cmd_list().await,
        CronCommand::Logs { name, tail, follow } => cmd_logs(name, tail, follow).await,
        CronCommand::Disable { name } => cmd_set_enabled(name, false).await,
        CronCommand::Enable { name } => cmd_set_enabled(name, true).await,
        CronCommand::Run { name } => cmd_run(name).await,
        CronCommand::Remove { name } => cmd_remove(name).await,
        CronCommand::Examples => {
            crate::commands::examples::run(crate::commands::examples::ExamplesArgs {
                command: Some("cron".into()),
            })
            .await
        }
        CronCommand::Other(args) => {
            Err(LynxError::unknown_command(args.first().map(|s| s.as_str()).unwrap_or(""), "cron").into())
        }
    }
}

// ── helpers ─────────────────────────────────────────────────────────────────

fn tasks_toml_path() -> std::path::PathBuf {
    lynx_core::paths::tasks_file()
}

fn task_logs_dir() -> std::path::PathBuf {
    lynx_core::paths::task_logs_dir()
}

/// Signal the daemon to reload via SIGHUP if a PID file exists.
fn signal_daemon_reload() {
    let Ok(pid_path) = lynx_core::runtime::pid_file() else {
        return;
    };
    if let Ok(content) = std::fs::read_to_string(&pid_path) {
        if let Ok(pid) = content.trim().parse::<u32>() {
            send_signal(pid as i32, 1); // SIGHUP = 1
        }
    }
}

#[cfg(unix)]
fn send_signal(pid: i32, sig: i32) {
    // Best-effort: SIGHUP to daemon may fail if not running
    let _ = std::process::Command::new("kill")
        .args([&format!("-{sig}"), &pid.to_string()])
        .status();
}

#[cfg(not(unix))]
fn send_signal(_pid: i32, _sig: i32) {}

// ── subcommand implementations ───────────────────────────────────────────────

async fn cmd_add(
    name: String,
    run: String,
    cron: String,
    description: String,
    on_fail_str: String,
    timeout: Option<String>,
    log: bool,
) -> Result<()> {
    let on_fail = match on_fail_str.as_str() {
        "notify" => OnFail::Notify,
        "ignore" => OnFail::Ignore,
        "log" | "" => OnFail::Log,
        other => return Err(LynxError::Task(format!("unknown on_fail value '{other}': use log, notify, or ignore")).into()),
    };

    let task = Task {
        name: name.clone(),
        description,
        run,
        cron,
        on_fail,
        timeout,
        log,
        enabled: true,
    };

    // Validate BEFORE writing to disk.
    validate_task(task.clone()).context("task validation failed")?;

    let path = tasks_toml_path();
    let content = read_tasks_file(&path)?;
    let mut file = parse_tasks_file(&content)?;

    if file.tasks.iter().any(|t| t.name == name) {
        return Err(LynxError::Task(format!("task '{name}' already exists — use 'lx cron remove {name}' first")).into());
    }

    file.tasks.push(task);
    write_tasks_file(&path, &file)?;
    signal_daemon_reload();

    println!("✓ task '{name}' added");
    Ok(())
}

struct CronListEntry {
    name: String,
    cron: String,
    enabled: bool,
    last_run: String,
    exit_code: String,
}

impl lynx_tui::ListItem for CronListEntry {
    fn title(&self) -> &str { &self.name }
    fn subtitle(&self) -> String {
        format!("{} {}", self.cron, if self.enabled { "" } else { "(disabled)" })
    }
    fn detail(&self) -> String {
        format!(
            "Schedule: {}\nEnabled: {}\nLast run: {}\nExit code: {}",
            self.cron, self.enabled, self.last_run, self.exit_code
        )
    }
    fn is_active(&self) -> bool { self.enabled }
}

async fn cmd_list() -> Result<()> {
    let path = tasks_toml_path();
    let content = read_tasks_file(&path)?;
    let file = parse_tasks_file(&content)?;

    if file.tasks.is_empty() {
        println!("No tasks configured. Use 'lx cron add' to create one.");
        return Ok(());
    }

    let log_dir = task_logs_dir();
    let entries: Vec<CronListEntry> = file.tasks.iter().map(|task| {
        let (last_run, exit_code) = read_last_run(&log_dir, &task.name);
        CronListEntry {
            name: task.name.clone(),
            cron: task.cron.clone(),
            enabled: task.enabled,
            last_run,
            exit_code,
        }
    }).collect();

    lynx_tui::show(&entries, "Cron Tasks", &super::tui_colors())?;
    Ok(())
}

async fn cmd_logs(name: String, tail_n: usize, follow: bool) -> Result<()> {
    let log_path = task_logs_dir().join(format!("{name}.jsonl"));

    if !log_path.exists() {
        println!("No logs found for task '{name}'.");
        return Ok(());
    }

    if follow {
        // Shell out to tail -f — platform-standard streaming.
        let status = std::process::Command::new("tail")
            .args(["-f", &log_path.to_string_lossy()])
            .status()
            .context("tail -f failed")?;

        if !status.success() {
            return Err(LynxError::Task("tail exited with error".into()).into());
        }
    } else {
        // Read last N lines.
        let file = std::fs::File::open(&log_path).context("failed to open log")?;
        let reader = BufReader::new(file);
        let lines: Vec<String> = reader.lines().map_while(Result::ok).collect();

        let start = lines.len().saturating_sub(tail_n);
        for line in &lines[start..] {
            // Pretty-print JSON entries.
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                let exit = match val["exit_code"].as_i64() {
                    Some(c) => format!("exit={c}"),
                    None if val["timed_out"].as_bool() == Some(true) => "timed_out".into(),
                    None => "exit=?".into(),
                };
                let duration = val["duration_ms"].as_u64().unwrap_or(0);
                let ts = val["started_at"].as_u64().unwrap_or(0);
                println!("[{ts}] {exit} duration={duration}ms");
                let stdout = val["stdout_tail"].as_str().unwrap_or("").trim();
                if !stdout.is_empty() {
                    println!("  stdout: {stdout}");
                }
                let stderr = val["stderr_tail"].as_str().unwrap_or("").trim();
                if !stderr.is_empty() {
                    println!("  stderr: {stderr}");
                }
            } else {
                println!("{line}");
            }
        }
    }

    Ok(())
}

async fn cmd_set_enabled(name: String, enabled: bool) -> Result<()> {
    let path = tasks_toml_path();
    let content = read_tasks_file(&path)?;
    let mut file = parse_tasks_file(&content)?;

    let task = file
        .tasks
        .iter_mut()
        .find(|t| t.name == name)
        .with_context(|| format!("task '{name}' not found"))?;

    task.enabled = enabled;
    write_tasks_file(&path, &file)?;
    signal_daemon_reload();

    let verb = if enabled { "resumed" } else { "paused" };
    println!("✓ task '{name}' {verb}");
    Ok(())
}

async fn cmd_run(name: String) -> Result<()> {
    let path = tasks_toml_path();
    let content = read_tasks_file(&path)?;
    let file = parse_tasks_file(&content)?;

    let task = file
        .tasks
        .iter()
        .find(|t| t.name == name)
        .with_context(|| format!("task '{name}' not found"))?;

    // Build a ValidatedTask and run it directly in this process.
    let vt = validate_task(task.clone()).context("task validation failed")?;
    let log_dir = task_logs_dir();
    std::fs::create_dir_all(&log_dir).context("failed to create log dir")?;

    println!("Running task '{name}'...");

    // Run via the scheduler's run_task function path — we shell out directly.
    let mut child = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(&vt.task.run)
        .spawn()
        .context("failed to spawn task")?;

    let status = if let Some(timeout) = vt.timeout_duration {
        match tokio::time::timeout(timeout, child.wait()).await {
            Ok(s) => s.context("wait failed")?,
            Err(_) => {
                let _ = child.kill().await; // best-effort: child may have exited
                println!("Task '{name}' timed out.");
                return Ok(());
            }
        }
    } else {
        child.wait().await.context("wait failed")?
    };

    let code = status.code().unwrap_or(-1);
    println!("Task '{name}' exited with code {code}.");

    Ok(())
}

async fn cmd_remove(name: String) -> Result<()> {
    let path = tasks_toml_path();
    let content = read_tasks_file(&path)?;
    let mut file = parse_tasks_file(&content)?;

    let before = file.tasks.len();
    file.tasks.retain(|t| t.name != name);

    if file.tasks.len() == before {
        return Err(LynxError::NotFound { item_type: "Task".into(), name: name.clone(), hint: "run `lx cron list` to see available tasks".into() }.into());
    }

    write_tasks_file(&path, &file)?;
    signal_daemon_reload();

    println!("✓ task '{name}' removed");
    Ok(())
}
