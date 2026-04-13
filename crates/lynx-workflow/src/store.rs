//! Workflow store — load and list workflow files from disk.

use crate::schema::{self, Workflow};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Summary entry returned by list_workflows().
#[derive(Debug, Clone)]
pub struct WorkflowEntry {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
}

/// Load a workflow by name from the workflows directory.
pub fn load_workflow(name: &str) -> Result<Workflow> {
    let dir = lynx_core::paths::workflows_dir();
    load_workflow_from(&dir, name)
}

/// Load a workflow by name from a specific directory.
pub fn load_workflow_from(dir: &Path, name: &str) -> Result<Workflow> {
    let path = dir.join(format!("{name}.toml"));
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("workflow '{}' not found at {}", name, path.display()))?;
    schema::parse(&content).with_context(|| format!("invalid workflow at {}", path.display()))
}

/// List all workflows in the workflows directory.
pub fn list_workflows() -> Result<Vec<WorkflowEntry>> {
    let dir = lynx_core::paths::workflows_dir();
    list_workflows_in(&dir)
}

/// List all workflows in a specific directory.
pub fn list_workflows_in(dir: &Path) -> Result<Vec<WorkflowEntry>> {
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut entries = Vec::new();
    for entry in std::fs::read_dir(dir)
        .with_context(|| format!("failed to read workflows dir {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        let description = match std::fs::read_to_string(&path) {
            Ok(content) => match schema::parse(&content) {
                Ok(wf) => wf.workflow.description,
                Err(_) => "(invalid)".into(),
            },
            Err(_) => "(unreadable)".into(),
        };

        entries.push(WorkflowEntry {
            name,
            description,
            path,
        });
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_workflow(dir: &Path, name: &str, content: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(format!("{name}.toml")), content).unwrap();
    }

    const VALID_WF: &str = r#"
        [workflow]
        name = "test"
        description = "A test workflow"

        [[step]]
        name = "hello"
        run = "echo hello"
    "#;

    #[test]
    fn load_from_temp_dir() {
        let tmp = tempfile::tempdir().unwrap();
        write_workflow(tmp.path(), "deploy", VALID_WF);
        let wf = load_workflow_from(tmp.path(), "deploy").unwrap();
        assert_eq!(wf.workflow.name, "test");
    }

    #[test]
    fn list_multiple_workflows() {
        let tmp = tempfile::tempdir().unwrap();
        write_workflow(tmp.path(), "alpha", VALID_WF);
        write_workflow(
            tmp.path(),
            "beta",
            r#"
            [workflow]
            name = "beta"
            description = "Beta workflow"
            [[step]]
            name = "s1"
            run = "echo beta"
        "#,
        );
        let entries = list_workflows_in(tmp.path()).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "alpha");
        assert_eq!(entries[1].name, "beta");
    }

    #[test]
    fn reject_invalid_toml() {
        let tmp = tempfile::tempdir().unwrap();
        write_workflow(tmp.path(), "bad", "not valid toml {{{{");
        let result = load_workflow_from(tmp.path(), "bad");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("bad.toml"), "error should include path: {err}");
    }

    #[test]
    fn missing_file_includes_path() {
        let tmp = tempfile::tempdir().unwrap();
        let result = load_workflow_from(tmp.path(), "nonexistent");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("nonexistent"),
            "error should include name: {err}"
        );
    }

    #[test]
    fn list_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let entries = list_workflows_in(tmp.path()).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn list_nonexistent_dir() {
        let entries = list_workflows_in(Path::new("/tmp/lynx-test-nonexistent-dir")).unwrap();
        assert!(entries.is_empty());
    }
}
