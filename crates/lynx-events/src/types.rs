//! Well-known event names emitted by the Lynx shell hooks.
//! New events go here — keeps all names in one place (D-008).

// ── Shell lifecycle ───────────────────────────────────────────────────────────
pub const SHELL_CHPWD: &str = "shell:chpwd";
pub const SHELL_PREEXEC: &str = "shell:preexec";
pub const SHELL_PRECMD: &str = "shell:precmd";
pub const SHELL_CONTEXT_CHANGED: &str = "shell:context-changed";

// ── Config / theme ────────────────────────────────────────────────────────────
pub const CONFIG_CHANGED: &str = "config:changed";
pub const THEME_CHANGED: &str = "theme:changed";

// ── Plugin lifecycle ──────────────────────────────────────────────────────────
pub const PLUGIN_LOADED: &str = "plugin:loaded";
pub const PLUGIN_UNLOADED: &str = "plugin:unloaded";
pub const PLUGIN_FAILED: &str = "plugin:failed";

// ── Git ───────────────────────────────────────────────────────────────────────
pub const GIT_BRANCH_CHANGED: &str = "git:branch-changed";
pub const GIT_STATE_UPDATED: &str = "git:state-updated";

// ── Task scheduler ────────────────────────────────────────────────────────────
pub const TASK_COMPLETED: &str = "task:completed";
pub const TASK_FAILED: &str = "task:failed";

/// Payload carried by an emitted event.
#[derive(Debug, Clone)]
pub struct Event {
    /// Well-known event name, e.g. `shell:chpwd`.
    pub name: String,
    /// Arbitrary string data associated with the event (may be empty).
    pub data: String,
}

impl Event {
    pub fn new(name: impl Into<String>, data: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            data: data.into(),
        }
    }

    pub fn named(name: impl Into<String>) -> Self {
        Self::new(name, "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_new_sets_fields() {
        let e = Event::new("shell:chpwd", "/home");
        assert_eq!(e.name, "shell:chpwd");
        assert_eq!(e.data, "/home");
    }

    #[test]
    fn event_named_empty_data() {
        let e = Event::named("plugin:loaded");
        assert_eq!(e.name, "plugin:loaded");
        assert!(e.data.is_empty());
    }

    #[test]
    fn event_new_from_string() {
        let e = Event::new(String::from("test"), String::from("payload"));
        assert_eq!(e.name, "test");
        assert_eq!(e.data, "payload");
    }

    #[test]
    fn well_known_event_names_are_namespaced() {
        let names = [
            SHELL_CHPWD,
            SHELL_PREEXEC,
            SHELL_PRECMD,
            SHELL_CONTEXT_CHANGED,
            CONFIG_CHANGED,
            THEME_CHANGED,
            PLUGIN_LOADED,
            PLUGIN_UNLOADED,
            PLUGIN_FAILED,
            GIT_BRANCH_CHANGED,
            GIT_STATE_UPDATED,
            TASK_COMPLETED,
            TASK_FAILED,
        ];
        for name in &names {
            assert!(
                name.contains(':'),
                "event name should be namespaced: {name}"
            );
        }
    }

    #[test]
    fn no_duplicate_event_names() {
        let names = [
            SHELL_CHPWD,
            SHELL_PREEXEC,
            SHELL_PRECMD,
            SHELL_CONTEXT_CHANGED,
            CONFIG_CHANGED,
            THEME_CHANGED,
            PLUGIN_LOADED,
            PLUGIN_UNLOADED,
            PLUGIN_FAILED,
            GIT_BRANCH_CHANGED,
            GIT_STATE_UPDATED,
            TASK_COMPLETED,
            TASK_FAILED,
        ];
        let mut seen = std::collections::HashSet::new();
        for name in &names {
            assert!(seen.insert(name), "duplicate event name: {name}");
        }
    }
}
