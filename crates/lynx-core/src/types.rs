use serde::{Deserialize, Serialize};

/// The context Lynx is running in — determines what gets loaded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Context {
    Interactive,  // normal shell use
    Agent,        // AI agentic coding (Claude Code, Cursor, etc.)
    Minimal,      // bare minimum — scripts, SSH, CI
}

impl Default for Context {
    fn default() -> Self {
        Self::Interactive
    }
}

/// Load strategy for a plugin or module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoadStrategy {
    Eager,   // load at startup
    Lazy,    // defer until first use
}

impl Default for LoadStrategy {
    fn default() -> Self {
        Self::Eager
    }
}
