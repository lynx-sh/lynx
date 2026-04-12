use serde::{Deserialize, Serialize};

/// Universal segment fields read by the evaluator before calling render.
/// Segments never need to handle these — the evaluator filters first.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SegmentVisibility {
    /// Only show this segment in these contexts. Overrides hide_in and defaults.
    pub show_in: Option<Vec<String>>,
    /// Hide this segment in these contexts. Ignored when show_in is set.
    pub hide_in: Option<Vec<String>>,
    /// Show this segment only when condition is true. Evaluated after show_in/hide_in.
    pub show_when: Option<SegmentCondition>,
    /// Hide this segment when condition is true. Ignored when show_when is set.
    pub hide_when: Option<SegmentCondition>,
}

/// A runtime condition evaluated against `RenderContext` — no I/O, no shell.
///
/// Exactly one field should be set per condition (untagged enum: first match wins).
/// TOML example:
/// ```toml
/// [segment.username]
/// show_when = { env_set = "SSH_CONNECTION" }
///
/// [segment.git_branch]
/// show_when = { in_git_repo = true }
///
/// [segment.venv]
/// show_when = { env_matches = { VIRTUAL_ENV = "*myproject*" } }
///
/// [segment.dir]
/// show_when = { cwd_matches = "~/work/**" }
///
/// [segment.exit_code]
/// show_when = { exit_code_nonzero = true }
/// ```
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum SegmentCondition {
    /// Segment visible only when the named env var is set (non-empty).
    EnvSet { env_set: String },
    /// Segment visible only when all listed env vars match their glob patterns.
    EnvMatches {
        env_matches: std::collections::HashMap<String, String>,
    },
    /// `true` = only in git repos; `false` = only outside git repos.
    InGitRepo { in_git_repo: bool },
    /// Segment visible only when cwd matches the glob pattern. `~` is expanded
    /// using the `HOME` env var from the render context.
    CwdMatches { cwd_matches: String },
    /// `true` = only when last exit code is non-zero; `false` = only on zero exit.
    ExitCodeNonzero { exit_code_nonzero: bool },
}

/// Shared color/style type — used by individual segment typed configs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SegmentColor {
    pub fg: Option<String>,
    #[serde(default)]
    pub bold: bool,
    pub bg: Option<String>,
}

/// Shared status icon type — used by git segment config.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct StatusIcon {
    pub icon: Option<String>,
    pub color: Option<String>,
}

/// The set of segment names that Lynx recognises. Used for validation.
pub const KNOWN_SEGMENTS: &[&str] = &[
    "aws_profile",
    "dir",
    "git_branch",
    "git_status",
    "git_action",
    "git_ahead_behind",
    "git_sha",
    "lang_version",
    "git_stash",
    "git_time_since_commit",
    "hist_number",
    "cmd_duration",
    "context_badge",
    "kubectl_context",
    "node_version",
    "ruby_version",
    "golang_version",
    "rust_version",
    "username",
    "hostname",
    "ssh_indicator",
    "venv",
    "conda_env",
    "task_status",
    "exit_code",
    "background_jobs",
    "vi_mode",
    "time",
    "newline",
    "prompt_char",
];
