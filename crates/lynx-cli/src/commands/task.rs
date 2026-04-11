use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand};
use lynx_task::{
    schema::{OnFail, Task, TasksFile}, validate_task,
};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

#[derive(Args)]
pub struct TaskArgs {
    #[command(subcommand)]
    pub command: TaskCommand,
}

#[derive(Subcommand)]
pub enum TaskCommand {
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
    Pause {
        /// Task name
        name: String,
    },
    /// Enable a paused task (set enabled=true)
    Resume {
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
}

pub async fn run(args: TaskArgs) -> Result<()> {
    match args.command {
        TaskCommand::Add {
            name, run, cron, description, on_fail, timeout, log,
        } => cmd_add(name, run, cron, description, on_fail, timeout, log).await,
        TaskCommand::List => cmd_list().await,
        TaskCommand::Logs { name, tail, follow } => cmd_logs(name, tail, follow).await,
        TaskCommand::Pause { name } => cmd_set_enabled(name, false).await,
        TaskCommand::Resume { name } => cmd_set_enabled(name, true).await,
        TaskCommand::Run { name } => cmd_run(name).await,
        TaskCommand::Remove { name } => cmd_remove(name).await,
        TaskCommand::Examples => {
            crate::commands::examples::run(
                crate::commands::examples::ExamplesArgs { command: Some("task".into()) }
            ).await
        }
    }
}

// ── helpers ─────────────────────────────────────────────────────────────────

fn tasks_toml_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".config/lynx/tasks.toml")
}

fn log_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".config/lynx/logs/tasks")
}

/// Load tasks.toml as a raw string (returns empty string if missing).
fn read_tasks_file(path: &Path) -> Result<String> {
    if !path.exists() {
        return Ok(String::new());
    }
    std::fs::read_to_string(path).context("failed to read tasks.toml")
}

/// Parse raw TOML string, preserving the full `TasksFile` structure.
fn parse_tasks_file(content: &str) -> Result<TasksFile> {
    if content.trim().is_empty() {
        return Ok(TasksFile::default());
    }
    toml::from_str::<TasksFile>(content).context("tasks.toml parse error")
}

/// Write a `TasksFile` back to disk.
fn write_tasks_file(path: &Path, file: &TasksFile) -> Result<()> {
    let parent = path.parent().unwrap_or(path);
    std::fs::create_dir_all(parent).context("failed to create config directory")?;

    let content = toml::to_string_pretty(file).context("failed to serialize tasks.toml")?;
    std::fs::write(path, content).context("failed to write tasks.toml")
}

/// Signal the daemon to reload via SIGHUP if a PID file exists.
fn signal_daemon_reload() {
    let Ok(pid_path) = lynx_core::runtime::pid_file() else {
        return;
    };
    if let Ok(content) = std::fs::read_to_string(&pid_path) {
        if let Ok(pid) = content.trim().parse::<u32>() {
            // Safety: SIGHUP is always safe to send to our own daemon.
            unsafe {
                libc_kill(pid as i32, 1); // SIGHUP = 1
            }
        }
    }
}

#[cfg(unix)]
unsafe fn libc_kill(pid: i32, sig: i32) {
    // Use inline syscall via std to avoid a libc dep.
    let _ = std::process::Command::new("kill")
        .args([&format!("-{sig}"), &pid.to_string()])
        .status();
}

#[cfg(not(unix))]
unsafe fn libc_kill(_pid: i32, _sig: i32) {}

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
        other => bail!("unknown on_fail value '{other}': use log, notify, or ignore"),
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
        bail!("task '{name}' already exists — use 'lx task remove {name}' first");
    }

    file.tasks.push(task);
    write_tasks_file(&path, &file)?;
    signal_daemon_reload();

    println!("✓ task '{name}' added");
    Ok(())
}

async fn cmd_list() -> Result<()> {
    let path = tasks_toml_path();
    let content = read_tasks_file(&path)?;
    let file = parse_tasks_file(&content)?;

    if file.tasks.is_empty() {
        println!("No tasks configured. Use 'lx task add' to create one.");
        return Ok(());
    }

    let log_dir = log_dir();

    println!(
        "{:<20} {:<18} {:<8} {:<12} {:<10}",
        "NAME", "LAST RUN", "EXIT", "ENABLED", "CRON"
    );
    println!("{}", "-".repeat(72));

    for task in &file.tasks {
        let (last_run, exit_code) = read_last_run(&log_dir, &task.name);
        println!(
            "{:<20} {:<18} {:<8} {:<12} {}",
            task.name,
            last_run,
            exit_code,
            if task.enabled { "yes" } else { "no" },
            task.cron,
        );
    }

    Ok(())
}

/// Read the last entry from a task's JSONL log. Returns ("never", "—") if no log.
fn read_last_run(log_dir: &Path, task_name: &str) -> (String, String) {
    let log_path = log_dir.join(format!("{task_name}.jsonl"));
    let Ok(file) = std::fs::File::open(&log_path) else {
        return ("never".into(), "—".into());
    };

    let reader = BufReader::new(file);
    let last = reader.lines().map_while(Result::ok).last();

    let Some(line) = last else {
        return ("never".into(), "—".into());
    };

    let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) else {
        return ("?".into(), "?".into());
    };

    let ts = val["started_at"].as_u64().unwrap_or(0);
    let exit = match val["exit_code"].as_i64() {
        Some(c) => c.to_string(),
        None if val["timed_out"].as_bool() == Some(true) => "timeout".into(),
        None => "?".into(),
    };

    let time_str = if ts == 0 {
        "?".into()
    } else {
        // Format as YYYY-MM-DD HH:MM
        let secs = ts;
        // Simple formatting without pulling in chrono just for display:
        // Use humantime to render elapsed instead.
        let elapsed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|now| now.as_secs().saturating_sub(secs))
            .unwrap_or(0);
        format!("{}s ago", elapsed)
    };

    (time_str, exit)
}

async fn cmd_logs(name: String, tail_n: usize, follow: bool) -> Result<()> {
    let log_path = log_dir().join(format!("{name}.jsonl"));

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
            bail!("tail exited with error");
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

    let task = file.tasks.iter_mut().find(|t| t.name == name)
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

    let task = file.tasks.iter().find(|t| t.name == name)
        .with_context(|| format!("task '{name}' not found"))?;

    // Build a ValidatedTask and run it directly in this process.
    let vt = validate_task(task.clone()).context("task validation failed")?;
    let log_dir = log_dir();
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
                let _ = child.kill().await;
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
        bail!("task '{name}' not found");
    }

    write_tasks_file(&path, &file)?;
    signal_daemon_reload();

    println!("✓ task '{name}' removed");
    Ok(())
}
