//! Runner registry — resolves runner types to binary + args.
//!
//! Runners do NOT execute commands. They resolve a run string into the
//! binary path and argument list that the step executor will spawn.

use crate::schema::RunnerType;
use anyhow::{bail, Result};

/// Resolved command ready for execution.
#[derive(Debug, Clone)]
pub struct ResolvedCommand {
    /// Binary to execute.
    pub binary: String,
    /// Arguments (the run string is typically the last arg).
    pub args: Vec<String>,
}

/// Resolve a RunnerType + run string into a binary and args.
pub fn resolve(runner: &RunnerType, run_str: &str) -> Result<ResolvedCommand> {
    match runner {
        RunnerType::Sh => Ok(ResolvedCommand {
            binary: "sh".into(),
            args: vec!["-c".into(), run_str.into()],
        }),
        RunnerType::Bash => Ok(ResolvedCommand {
            binary: "bash".into(),
            args: vec!["-c".into(), run_str.into()],
        }),
        RunnerType::Zsh => Ok(ResolvedCommand {
            binary: "zsh".into(),
            args: vec!["-c".into(), run_str.into()],
        }),
        RunnerType::Python => Ok(ResolvedCommand {
            binary: "python3".into(),
            args: vec!["-c".into(), run_str.into()],
        }),
        RunnerType::Node => Ok(ResolvedCommand {
            binary: "node".into(),
            args: vec!["-e".into(), run_str.into()],
        }),
        RunnerType::Go => Ok(ResolvedCommand {
            binary: "go".into(),
            args: vec!["run".into(), run_str.into()],
        }),
        RunnerType::Cargo => {
            // Parse: "cargo build --release" → binary=cargo, args=[build, --release]
            let parts: Vec<&str> = run_str.split_whitespace().collect();
            if parts.is_empty() {
                bail!("cargo runner: empty run string");
            }
            // If run_str starts with "cargo", strip it
            let args = if parts[0] == "cargo" {
                parts[1..].iter().map(|s| s.to_string()).collect()
            } else {
                parts.iter().map(|s| s.to_string()).collect()
            };
            Ok(ResolvedCommand {
                binary: "cargo".into(),
                args,
            })
        }
        RunnerType::Curl => Ok(ResolvedCommand {
            binary: "curl".into(),
            args: shell_split(run_str),
        }),
        RunnerType::Docker => Ok(ResolvedCommand {
            binary: "docker".into(),
            args: shell_split(run_str),
        }),
        RunnerType::Custom(name) => {
            // Custom runners are resolved as: the custom name is the binary
            Ok(ResolvedCommand {
                binary: name.clone(),
                args: shell_split(run_str),
            })
        }
    }
}

/// Simple whitespace split for args (no quote handling — use sh -c for complex cases).
fn shell_split(s: &str) -> Vec<String> {
    s.split_whitespace().map(String::from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_sh() {
        let cmd = resolve(&RunnerType::Sh, "echo hello").unwrap();
        assert_eq!(cmd.binary, "sh");
        assert_eq!(cmd.args, vec!["-c", "echo hello"]);
    }

    #[test]
    fn resolve_bash() {
        let cmd = resolve(&RunnerType::Bash, "set -e && deploy").unwrap();
        assert_eq!(cmd.binary, "bash");
        assert_eq!(cmd.args, vec!["-c", "set -e && deploy"]);
    }

    #[test]
    fn resolve_zsh() {
        let cmd = resolve(&RunnerType::Zsh, "source ~/.zshrc").unwrap();
        assert_eq!(cmd.binary, "zsh");
    }

    #[test]
    fn resolve_python() {
        let cmd = resolve(&RunnerType::Python, "print('hi')").unwrap();
        assert_eq!(cmd.binary, "python3");
        assert_eq!(cmd.args, vec!["-c", "print('hi')"]);
    }

    #[test]
    fn resolve_node() {
        let cmd = resolve(&RunnerType::Node, "console.log('hi')").unwrap();
        assert_eq!(cmd.binary, "node");
        assert_eq!(cmd.args, vec!["-e", "console.log('hi')"]);
    }

    #[test]
    fn resolve_go() {
        let cmd = resolve(&RunnerType::Go, "main.go").unwrap();
        assert_eq!(cmd.binary, "go");
        assert_eq!(cmd.args, vec!["run", "main.go"]);
    }

    #[test]
    fn resolve_cargo() {
        let cmd = resolve(&RunnerType::Cargo, "cargo build --release").unwrap();
        assert_eq!(cmd.binary, "cargo");
        assert_eq!(cmd.args, vec!["build", "--release"]);
    }

    #[test]
    fn resolve_cargo_without_prefix() {
        let cmd = resolve(&RunnerType::Cargo, "test --all").unwrap();
        assert_eq!(cmd.binary, "cargo");
        assert_eq!(cmd.args, vec!["test", "--all"]);
    }

    #[test]
    fn resolve_curl() {
        let cmd = resolve(&RunnerType::Curl, "-s https://example.com").unwrap();
        assert_eq!(cmd.binary, "curl");
        assert_eq!(cmd.args, vec!["-s", "https://example.com"]);
    }

    #[test]
    fn resolve_docker() {
        let cmd = resolve(&RunnerType::Docker, "run --rm alpine echo hi").unwrap();
        assert_eq!(cmd.binary, "docker");
        assert_eq!(cmd.args, vec!["run", "--rm", "alpine", "echo", "hi"]);
    }

    #[test]
    fn resolve_custom() {
        let cmd = resolve(&RunnerType::Custom("my-tool".into()), "--flag value").unwrap();
        assert_eq!(cmd.binary, "my-tool");
        assert_eq!(cmd.args, vec!["--flag", "value"]);
    }
}
