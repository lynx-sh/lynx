//! Workflow TOML schema — data definitions for workflow files (D-031).
//!
//! Workflows are TOML data, not code. Each step declares a runner and a
//! run string. Lynx orchestrates execution order, concurrency, and signals.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A complete workflow definition parsed from TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    /// Workflow metadata.
    pub workflow: WorkflowMeta,
    /// Ordered list of steps.
    #[serde(rename = "step", default)]
    pub steps: Vec<Step>,
}

/// Workflow-level metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowMeta {
    /// Unique workflow name.
    pub name: String,
    /// Human-readable description.
    #[serde(default)]
    pub description: String,
    /// Optional context restriction (e.g. "interactive" only).
    #[serde(default)]
    pub context: Option<String>,
    /// Typed parameters the workflow accepts.
    #[serde(rename = "param", default)]
    pub params: Vec<WorkflowParam>,
}

/// A typed parameter for parameterized workflows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowParam {
    /// Parameter name (used as `$param_name` in step run strings).
    pub name: String,
    /// Parameter type.
    #[serde(rename = "type", default)]
    pub param_type: ParamType,
    /// Whether the parameter is required (default: true).
    #[serde(default = "default_true")]
    pub required: bool,
    /// Default value if not provided.
    #[serde(default)]
    pub default: Option<String>,
    /// Allowed values (empty = any).
    #[serde(default)]
    pub choices: Vec<String>,
    /// Help text shown in prompts.
    #[serde(default)]
    pub description: String,
}

/// Supported parameter types.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ParamType {
    #[default]
    String,
    Int,
    Bool,
}

/// A single workflow step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    /// Step name (unique within workflow).
    pub name: String,
    /// Runner to use (sh, bash, zsh, python, node, go, cargo, curl, docker).
    #[serde(default)]
    pub runner: RunnerType,
    /// Command or script to execute.
    pub run: String,
    /// If true, prompt for confirmation before executing.
    #[serde(default)]
    pub confirm: bool,
    /// Timeout in seconds (None = no timeout).
    #[serde(default)]
    pub timeout_sec: Option<u64>,
    /// What to do on failure.
    #[serde(default)]
    pub on_fail: OnFail,
    /// Number of retry attempts on failure (only with OnFail::Retry).
    #[serde(default)]
    pub retry_count: u32,
    /// Condition expression — step runs only if this evaluates truthy.
    /// Simple form: `"$param_name == value"` or `"env:VAR"` (non-empty check).
    #[serde(default)]
    pub condition: Option<String>,
    /// Steps that must complete before this one starts.
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Concurrency group name — steps in the same group run in parallel.
    #[serde(default)]
    pub group: Option<String>,
    /// Extra environment variables for this step.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Working directory for this step (default: current directory).
    #[serde(default)]
    pub cwd: Option<String>,
}

/// Built-in runner types and custom runner escape hatch.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RunnerType {
    #[default]
    Sh,
    Bash,
    Zsh,
    Python,
    Node,
    Go,
    Cargo,
    Curl,
    Docker,
    /// Custom runner registered via plugin.toml.
    #[serde(untagged)]
    Custom(String),
}

/// What to do when a step fails.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OnFail {
    /// Abort the entire workflow.
    #[default]
    Abort,
    /// Retry the step up to retry_count times.
    Retry,
    /// Skip the failed step and continue.
    Continue,
}

fn default_true() -> bool {
    true
}

/// Parse a workflow TOML string into a Workflow.
pub fn parse(content: &str) -> anyhow::Result<Workflow> {
    let wf: Workflow = toml::from_str(content)?;
    validate(&wf)?;
    Ok(wf)
}

/// Validate a parsed workflow.
pub fn validate(wf: &Workflow) -> anyhow::Result<()> {
    if wf.workflow.name.is_empty() {
        anyhow::bail!("workflow name is required");
    }
    if wf.steps.is_empty() {
        anyhow::bail!("workflow must have at least one step");
    }

    let mut seen_names = std::collections::HashSet::new();
    for step in &wf.steps {
        if step.name.is_empty() {
            anyhow::bail!("step name is required");
        }
        if !seen_names.insert(&step.name) {
            anyhow::bail!("duplicate step name: '{}'", step.name);
        }
        if step.run.is_empty() {
            anyhow::bail!("step '{}': run is required", step.name);
        }
        // Validate depends_on references
        for dep in &step.depends_on {
            if !wf.steps.iter().any(|s| &s.name == dep) {
                anyhow::bail!(
                    "step '{}': depends_on '{}' does not exist",
                    step.name,
                    dep
                );
            }
        }
    }

    // Validate params
    for param in &wf.workflow.params {
        if param.name.is_empty() {
            anyhow::bail!("param name is required");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_workflow() {
        let toml = r#"
            [workflow]
            name = "deploy"
            description = "Deploy to production"

            [[step]]
            name = "build"
            run = "cargo build --release"
        "#;
        let wf = parse(toml).unwrap();
        assert_eq!(wf.workflow.name, "deploy");
        assert_eq!(wf.steps.len(), 1);
        assert_eq!(wf.steps[0].runner, RunnerType::Sh);
        assert_eq!(wf.steps[0].on_fail, OnFail::Abort);
    }

    #[test]
    fn parse_workflow_with_params() {
        let toml = r#"
            [workflow]
            name = "release"

            [[workflow.param]]
            name = "version"
            type = "string"
            required = true

            [[workflow.param]]
            name = "dry_run"
            type = "bool"
            required = false
            default = "true"

            [[step]]
            name = "tag"
            run = "git tag v$version"
        "#;
        let wf = parse(toml).unwrap();
        assert_eq!(wf.workflow.params.len(), 2);
        assert_eq!(wf.workflow.params[0].param_type, ParamType::String);
        assert!(wf.workflow.params[0].required);
        assert_eq!(wf.workflow.params[1].param_type, ParamType::Bool);
        assert!(!wf.workflow.params[1].required);
    }

    #[test]
    fn parse_concurrent_groups() {
        let toml = r#"
            [workflow]
            name = "parallel"

            [[step]]
            name = "lint"
            run = "cargo clippy"
            group = "checks"

            [[step]]
            name = "test"
            run = "cargo test"
            group = "checks"

            [[step]]
            name = "deploy"
            run = "deploy.sh"
            depends_on = ["lint", "test"]
        "#;
        let wf = parse(toml).unwrap();
        assert_eq!(wf.steps[0].group, Some("checks".into()));
        assert_eq!(wf.steps[1].group, Some("checks".into()));
        assert_eq!(wf.steps[2].depends_on, vec!["lint", "test"]);
    }

    #[test]
    fn parse_all_runners() {
        for runner in ["sh", "bash", "zsh", "python", "node", "go", "cargo", "curl", "docker"] {
            let toml = format!(
                r#"
                [workflow]
                name = "test"
                [[step]]
                name = "s1"
                runner = "{runner}"
                run = "echo hi"
            "#
            );
            let wf = parse(&toml).unwrap();
            assert!(!format!("{:?}", wf.steps[0].runner).is_empty());
        }
    }

    #[test]
    fn parse_step_with_all_fields() {
        let toml = r#"
            [workflow]
            name = "full"

            [[step]]
            name = "deploy"
            runner = "bash"
            run = "./deploy.sh"
            confirm = true
            timeout_sec = 300
            on_fail = "retry"
            retry_count = 3
            condition = "$env == prod"
            depends_on = []
            group = "deploy-group"
            cwd = "/app"
            [step.env]
            DEPLOY_ENV = "production"
        "#;
        let wf = parse(toml).unwrap();
        let s = &wf.steps[0];
        assert_eq!(s.runner, RunnerType::Bash);
        assert!(s.confirm);
        assert_eq!(s.timeout_sec, Some(300));
        assert_eq!(s.on_fail, OnFail::Retry);
        assert_eq!(s.retry_count, 3);
        assert_eq!(s.condition, Some("$env == prod".into()));
        assert_eq!(s.group, Some("deploy-group".into()));
        assert_eq!(s.cwd, Some("/app".into()));
        assert_eq!(s.env.get("DEPLOY_ENV").unwrap(), "production");
    }

    #[test]
    fn reject_empty_name() {
        let toml = r#"
            [workflow]
            name = ""
            [[step]]
            name = "s1"
            run = "echo hi"
        "#;
        assert!(parse(toml).is_err());
    }

    #[test]
    fn reject_no_steps() {
        let toml = r#"
            [workflow]
            name = "empty"
        "#;
        assert!(parse(toml).is_err());
    }

    #[test]
    fn reject_duplicate_step_names() {
        let toml = r#"
            [workflow]
            name = "dup"
            [[step]]
            name = "s1"
            run = "echo 1"
            [[step]]
            name = "s1"
            run = "echo 2"
        "#;
        assert!(parse(toml).is_err());
    }

    #[test]
    fn reject_missing_run() {
        let toml = r#"
            [workflow]
            name = "norun"
            [[step]]
            name = "s1"
            run = ""
        "#;
        assert!(parse(toml).is_err());
    }

    #[test]
    fn reject_invalid_depends_on() {
        let toml = r#"
            [workflow]
            name = "baddep"
            [[step]]
            name = "s1"
            run = "echo hi"
            depends_on = ["nonexistent"]
        "#;
        assert!(parse(toml).is_err());
    }

    #[test]
    fn roundtrip_serialization() {
        let toml = r#"
            [workflow]
            name = "roundtrip"
            description = "test"

            [[step]]
            name = "build"
            runner = "cargo"
            run = "cargo build"
            on_fail = "continue"
        "#;
        let wf = parse(toml).unwrap();
        let serialized = toml::to_string_pretty(&wf).unwrap();
        let wf2 = parse(&serialized).unwrap();
        assert_eq!(wf2.workflow.name, wf.workflow.name);
        assert_eq!(wf2.steps.len(), wf.steps.len());
    }
}
