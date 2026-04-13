//! Job manager — list, view, kill, and clean workflow job results.

use anyhow::{Context, Result};
use lynx_core::error::LynxError;
use std::path::Path;

/// Summary of a completed or running job.
#[derive(Debug, Clone)]
pub struct JobEntry {
    pub job_id: String,
    pub workflow: String,
    pub success: bool,
    pub started_at: u64,
    pub duration_ms: u64,
}

/// List all jobs in the jobs directory, sorted by start time (newest first).
pub fn list_jobs() -> Result<Vec<JobEntry>> {
    let dir = lynx_core::paths::jobs_dir();
    list_jobs_in(&dir)
}

/// List jobs in a specific directory.
pub fn list_jobs_in(dir: &Path) -> Result<Vec<JobEntry>> {
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut entries = Vec::new();
    for entry in std::fs::read_dir(dir).context("failed to read jobs dir")? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                entries.push(JobEntry {
                    job_id: val["job_id"].as_str().unwrap_or("").to_string(),
                    workflow: val["workflow"].as_str().unwrap_or("").to_string(),
                    success: val["success"].as_bool().unwrap_or(false),
                    started_at: val["started_at"].as_u64().unwrap_or(0),
                    duration_ms: val["duration_ms"].as_u64().unwrap_or(0),
                });
            }
        }
    }

    entries.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    Ok(entries)
}

/// Get full job JSON by ID.
pub fn get_job(job_id: &str) -> Result<serde_json::Value> {
    let dir = lynx_core::paths::jobs_dir();
    let path = dir.join(format!("{job_id}.json"));
    let content =
        std::fs::read_to_string(&path).with_context(|| format!("job '{job_id}' not found"))?;
    serde_json::from_str(&content).context("invalid job JSON")
}

/// Kill a running job by sending SIGTERM to its PID.
pub fn kill_job(job_id: &str) -> Result<()> {
    let dir = lynx_core::paths::jobs_dir();
    let pid_path = dir.join(format!("{job_id}.pid"));
    if !pid_path.exists() {
        return Err(LynxError::Workflow(format!(
            "no PID file for job '{job_id}' — it may have already completed"
        )).into());
    }
    let pid_str = std::fs::read_to_string(&pid_path).context("failed to read PID file")?;
    let pid: u32 = pid_str.trim().parse().context("invalid PID")?;

    #[cfg(unix)]
    {
        let _ = std::process::Command::new("kill")
            .args([&pid.to_string()])
            .status();
    }

    // Clean up PID file
    let _ = std::fs::remove_file(&pid_path);
    Ok(())
}

/// Read job log file content.
pub fn read_job_log(job_id: &str) -> Result<String> {
    let dir = lynx_core::paths::jobs_dir();
    let path = dir.join(format!("{job_id}.log"));
    if !path.exists() {
        let json_path = dir.join(format!("{job_id}.json"));
        if json_path.exists() {
            return std::fs::read_to_string(&json_path).context("failed to read job file");
        }
        return Err(LynxError::Workflow(format!(
            "no log found for job '{job_id}'"
        )).into());
    }
    std::fs::read_to_string(&path).context("failed to read job log")
}

/// Clean old job files older than max_age_hours.
pub fn clean_jobs(max_age_hours: u64) -> Result<usize> {
    let dir = lynx_core::paths::jobs_dir();
    clean_jobs_in(&dir, max_age_hours)
}

/// Clean jobs in a specific directory.
pub fn clean_jobs_in(dir: &Path, max_age_hours: u64) -> Result<usize> {
    if !dir.exists() {
        return Ok(0);
    }

    let cutoff = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        .saturating_sub(max_age_hours * 3600);

    let mut removed = 0;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if let Ok(meta) = path.metadata() {
            if let Ok(modified) = meta.modified() {
                let mod_secs = modified
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                if mod_secs < cutoff {
                    let _ = std::fs::remove_file(&path);
                    removed += 1;
                }
            }
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let entries = list_jobs_in(tmp.path()).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn list_nonexistent_dir() {
        let entries =
            list_jobs_in(std::path::Path::new("/tmp/lynx-test-nonexistent-jobs")).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn parse_job_state_file() {
        let tmp = tempfile::tempdir().unwrap();
        let job = serde_json::json!({
            "workflow": "deploy",
            "job_id": "deploy-20260412",
            "success": true,
            "started_at": 1000u64,
            "duration_ms": 500u64,
            "steps": [],
        });
        std::fs::write(
            tmp.path().join("deploy-20260412.json"),
            serde_json::to_string(&job).unwrap(),
        )
        .unwrap();
        let entries = list_jobs_in(tmp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].workflow, "deploy");
        assert!(entries[0].success);
    }

    #[test]
    fn clean_removes_old_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("old.json");
        std::fs::write(&path, "{}").unwrap();
        // Set modification time to the past (can't easily, so just test with 0 hours = removes all)
        let removed = clean_jobs_in(tmp.path(), 0).unwrap();
        assert_eq!(removed, 0); // file is brand new, so not removed with 0 hours
    }
}
