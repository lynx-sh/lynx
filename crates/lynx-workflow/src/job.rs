//! Job result persistence and execution helpers.

use crate::executor::JobResult;
use crate::schema::Step;
use std::collections::HashMap;

/// Evaluate a simple condition string against params.
pub fn evaluate_condition(condition: &str, params: &HashMap<String, String>) -> bool {
    let condition = condition.trim();
    if condition.is_empty() {
        return true;
    }
    // env:VAR — check if environment variable is set and non-empty
    if let Some(var) = condition.strip_prefix("env:") {
        return std::env::var(var).map(|v| !v.is_empty()).unwrap_or(false);
    }
    // $param == value
    if let Some((lhs, rhs)) = condition.split_once("==") {
        let lhs = lhs.trim();
        let rhs = rhs.trim();
        let lhs_val = if let Some(param) = lhs.strip_prefix('$') {
            params.get(param).map(|s| s.as_str()).unwrap_or("")
        } else {
            lhs
        };
        return lhs_val == rhs;
    }
    // $param — truthy check (non-empty)
    if let Some(param) = condition.strip_prefix('$') {
        return params.get(param).map(|v| !v.is_empty()).unwrap_or(false);
    }
    true
}

/// Substitute $param_name in run string with actual values.
pub fn substitute_params(run_str: &str, params: &HashMap<String, String>) -> String {
    let mut result = run_str.to_string();
    for (key, value) in params {
        result = result.replace(&format!("${key}"), value);
    }
    result
}

/// Build execution plan: group consecutive steps with same group for parallel exec.
pub fn build_plan(steps: &[Step]) -> Vec<Vec<Step>> {
    let mut plan: Vec<Vec<Step>> = Vec::new();
    let mut i = 0;

    while i < steps.len() {
        if let Some(ref group) = steps[i].group {
            let mut batch = vec![steps[i].clone()];
            let mut j = i + 1;
            while j < steps.len() && steps[j].group.as_ref() == Some(group) {
                batch.push(steps[j].clone());
                j += 1;
            }
            plan.push(batch);
            i = j;
        } else {
            plan.push(vec![steps[i].clone()]);
            i += 1;
        }
    }
    plan
}

pub fn generate_job_id(workflow_name: &str) -> String {
    let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    format!("{workflow_name}-{ts}")
}

pub fn epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn persist_job_result(result: &JobResult, log_dir: &std::path::Path) {
    if let Err(e) = std::fs::create_dir_all(log_dir) {
        tracing::warn!("failed to create job log dir {}: {e}", log_dir.display());
        return;
    }
    let path = log_dir.join(format!("{}.json", result.job_id));
    let json = serde_json::json!({
        "workflow": result.workflow_name,
        "job_id": result.job_id,
        "success": result.success,
        "started_at": result.started_at,
        "duration_ms": result.duration_ms,
        "steps": result.steps.iter().map(|s| serde_json::json!({
            "name": s.name,
            "status": format!("{:?}", s.status).to_lowercase(),
            "exit_code": s.exit_code,
            "duration_ms": s.duration_ms,
        })).collect::<Vec<_>>(),
    });
    if let Err(e) = std::fs::write(&path, serde_json::to_string_pretty(&json).unwrap_or_default()) {
        tracing::warn!("failed to persist job result to {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{OnFail, RunnerType};

    fn make_step(name: &str, run: &str) -> Step {
        Step {
            name: name.into(),
            runner: RunnerType::Sh,
            run: run.into(),
            confirm: false,
            timeout_sec: None,
            on_fail: OnFail::Abort,
            retry_count: 0,
            condition: None,
            depends_on: vec![],
            group: None,
            env: HashMap::new(),
            cwd: None,
        }
    }

    #[test]
    fn param_substitution() {
        let mut params = HashMap::new();
        params.insert("version".into(), "1.2.3".into());
        assert_eq!(substitute_params("echo $version", &params), "echo 1.2.3");
    }

    #[test]
    fn build_plan_groups_consecutive() {
        let mut s1 = make_step("a", "true");
        s1.group = Some("g".into());
        let mut s2 = make_step("b", "true");
        s2.group = Some("g".into());
        let s3 = make_step("c", "true");
        let plan = build_plan(&[s1, s2, s3]);
        assert_eq!(plan.len(), 2);
        assert_eq!(plan[0].len(), 2);
        assert_eq!(plan[1].len(), 1);
    }

    #[test]
    fn evaluate_empty_condition() {
        assert!(evaluate_condition("", &HashMap::new()));
    }

    #[test]
    fn evaluate_param_truthy() {
        let mut params = HashMap::new();
        params.insert("flag".into(), "yes".into());
        assert!(evaluate_condition("$flag", &params));
        assert!(!evaluate_condition("$missing", &params));
    }
}
