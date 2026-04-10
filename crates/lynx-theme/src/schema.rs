use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Top-level theme file structure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Theme {
    pub meta: ThemeMeta,
    pub segments: SegmentLayout,
    #[serde(default)]
    pub segment: HashMap<String, SegmentConfig>,
    #[serde(default)]
    pub colors: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemeMeta {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SegmentLayout {
    #[serde(default)]
    pub left: SegmentOrder,
    #[serde(default)]
    pub right: SegmentOrder,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SegmentOrder {
    #[serde(default)]
    pub order: Vec<String>,
}

/// Per-segment configuration — all fields optional; missing fields fall back to
/// segment defaults at render time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SegmentConfig {
    // dir segment
    pub max_depth: Option<u32>,
    pub truncate_to_repo: Option<bool>,

    // git_branch segment
    pub icon: Option<String>,

    // git_status icons
    pub staged: Option<StatusIcon>,
    pub modified: Option<StatusIcon>,
    pub untracked: Option<StatusIcon>,

    // cmd_duration segment
    pub min_ms: Option<u64>,

    // context_badge segment
    pub show_in: Option<Vec<String>>,
    pub label: Option<HashMap<String, String>>,

    // general color/style
    pub color: Option<SegmentColor>,

    // segment-level cache TTL (ms)
    pub cache_ttl_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct StatusIcon {
    pub icon: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SegmentColor {
    pub fg: Option<String>,
    #[serde(default)]
    pub bold: bool,
    pub bg: Option<String>,
}

/// The set of segment names that Lynx recognises. Used for validation.
pub const KNOWN_SEGMENTS: &[&str] = &[
    "dir",
    "git_branch",
    "git_status",
    "cmd_duration",
    "context_badge",
];
