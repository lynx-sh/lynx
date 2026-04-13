use serde::{Deserialize, Serialize};

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Parsed representation of a plugin.toml file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginManifest {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub plugin: PluginMeta,
    #[serde(default)]
    pub load: LoadConfig,
    #[serde(default)]
    pub deps: DepsConfig,
    #[serde(default)]
    pub exports: ExportsConfig,
    #[serde(default)]
    pub contexts: ContextsConfig,
    #[serde(default)]
    pub state: StateConfig,
    /// Shell integration config — ZLE widgets, keybindings, and fpath entries.
    #[serde(default)]
    pub shell: ShellConfig,
}

/// Shell integration config for a plugin.
///
/// Emitted by `lx plugin exec` as eval-able zsh during plugin load.
/// All paths in `fpath` are relative to the plugin directory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ShellConfig {
    /// Directories to prepend to `$fpath` (relative to plugin dir).
    /// Convention: `completions/` for zsh completion files.
    /// Emitted as `fpath=("$LYNX_PLUGIN_DIR/<dir>" $fpath)` before init.zsh is sourced.
    #[serde(default)]
    pub fpath: Vec<String>,
    /// ZLE widgets to register with `zle -N`.
    /// The widget function must be defined in functions.zsh and listed in exports.functions.
    #[serde(default)]
    pub widgets: Vec<String>,
    /// Key bindings to register with `bindkey`.
    /// Each entry binds a key sequence to a ZLE widget.
    #[serde(default)]
    pub keybindings: Vec<KeyBinding>,
    /// Set to true for plugins that hook into ZLE (e.g. zsh-syntax-highlighting,
    /// zsh-autosuggestions). These must be sourced directly — zle -N fails inside eval.
    #[serde(default)]
    pub zle_hook: bool,
}

/// A single key → widget binding emitted as `bindkey '<key>' <widget>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyBinding {
    /// The key sequence (e.g. `"^F"`, `"\\eOA"`, `"${terminfo[kcuu1]}"`).
    pub key: String,
    /// The ZLE widget to invoke (must be declared in `shell.widgets`).
    pub widget: String,
}

fn default_schema_version() -> u32 {
    CURRENT_SCHEMA_VERSION
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginMeta {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub authors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LoadConfig {
    #[serde(default)]
    pub lazy: bool,
    #[serde(default)]
    pub hooks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DepsConfig {
    #[serde(default)]
    pub binaries: Vec<String>,
    #[serde(default)]
    pub plugins: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ExportsConfig {
    #[serde(default)]
    pub functions: Vec<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ContextsConfig {
    #[serde(default)]
    pub disabled_in: Vec<String>,
}

/// State-gathering configuration for third-party plugins (D-014).
///
/// First-party plugins (git, kubectl) use native Rust gatherers inside `lx`
/// and leave this empty. Community plugins in any language set `gather` to a
/// shell command whose stdout is evaled by `lx refresh-state` each precmd.
///
/// The gather command contract:
/// - Stdout must be valid zsh (safe to pass to `eval`)
/// - Should export a `LYNX_CACHE_<NAME>_STATE` env var containing JSON
/// - Should set a `_lynx_<name>_state` zsh assoc array for shell helper functions
/// - Must be silent on failure (no stderr noise in the prompt)
/// - Must complete in under 200ms or the prompt will feel slow
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct StateConfig {
    /// Shell command to run for state gathering. Evaled each precmd.
    /// May reference `$PLUGIN_DIR` for the plugin's install directory.
    /// Example: `"my-plugin state"` or `"python3 $PLUGIN_DIR/state.py"`
    #[serde(default)]
    pub gather: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_manifest_deserialize() {
        let toml = r#"
            [plugin]
            name = "test"
            version = "1.0.0"
        "#;
        let m: PluginManifest = toml::from_str(toml).unwrap();
        assert_eq!(m.plugin.name, "test");
        assert_eq!(m.plugin.version, "1.0.0");
        assert!(!m.load.lazy);
        assert!(m.load.hooks.is_empty());
        assert!(m.deps.binaries.is_empty());
        assert!(m.exports.functions.is_empty());
        assert!(m.contexts.disabled_in.is_empty());
        assert!(m.state.gather.is_none());
        assert!(!m.shell.zle_hook);
    }

    #[test]
    fn full_manifest_deserialize() {
        let toml = r#"
            schema_version = 1
            [plugin]
            name = "git"
            version = "2.0.0"
            description = "Git integration"
            authors = ["Alice", "Bob"]

            [load]
            lazy = true
            hooks = ["chpwd", "precmd"]

            [deps]
            binaries = ["git"]
            plugins = ["core"]

            [exports]
            functions = ["git_status", "git_branch"]
            aliases = ["gs", "gb"]

            [contexts]
            disabled_in = ["agent", "minimal"]

            [state]
            gather = "git-state gather"

            [shell]
            fpath = ["completions"]
            widgets = ["my-widget"]
            zle_hook = true

            [[shell.keybindings]]
            key = "^F"
            widget = "my-widget"
        "#;
        let m: PluginManifest = toml::from_str(toml).unwrap();
        assert_eq!(m.schema_version, 1);
        assert_eq!(m.plugin.name, "git");
        assert_eq!(m.plugin.authors, vec!["Alice", "Bob"]);
        assert!(m.load.lazy);
        assert_eq!(m.load.hooks, vec!["chpwd", "precmd"]);
        assert_eq!(m.deps.binaries, vec!["git"]);
        assert_eq!(m.deps.plugins, vec!["core"]);
        assert_eq!(m.exports.functions, vec!["git_status", "git_branch"]);
        assert_eq!(m.exports.aliases, vec!["gs", "gb"]);
        assert_eq!(m.contexts.disabled_in, vec!["agent", "minimal"]);
        assert_eq!(m.state.gather.as_deref(), Some("git-state gather"));
        assert!(m.shell.zle_hook);
        assert_eq!(m.shell.fpath, vec!["completions"]);
        assert_eq!(m.shell.widgets, vec!["my-widget"]);
        assert_eq!(m.shell.keybindings.len(), 1);
        assert_eq!(m.shell.keybindings[0].key, "^F");
    }

    #[test]
    fn manifest_serialize_roundtrip() {
        let m = PluginManifest {
            schema_version: 1,
            plugin: PluginMeta {
                name: "test".into(),
                version: "1.0.0".into(),
                description: "desc".into(),
                authors: vec!["a".into()],
            },
            load: LoadConfig {
                lazy: false,
                hooks: vec![],
            },
            deps: DepsConfig::default(),
            exports: ExportsConfig {
                functions: vec!["f".into()],
                aliases: vec![],
            },
            contexts: ContextsConfig {
                disabled_in: vec!["agent".into()],
            },
            state: StateConfig::default(),
            shell: ShellConfig::default(),
        };
        let toml_str = toml::to_string_pretty(&m).unwrap();
        let back: PluginManifest = toml::from_str(&toml_str).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn default_schema_version_is_current() {
        assert_eq!(default_schema_version(), CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn shell_config_default() {
        let sc = ShellConfig::default();
        assert!(sc.fpath.is_empty());
        assert!(sc.widgets.is_empty());
        assert!(sc.keybindings.is_empty());
        assert!(!sc.zle_hook);
    }
}
