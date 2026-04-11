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
}

/// A single key → widget binding emitted as `bindkey '<key>' <widget>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyBinding {
    /// The key sequence (e.g. `"^F"`, `"\\eOA"`, `"${terminfo[kcuu1]}"`).
    pub key: String,
    /// The ZLE widget to invoke (must be declared in `shell.widgets`).
    pub widget: String,
}

fn default_schema_version() -> u32 {
    CURRENT_SCHEMA_VERSION
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginMeta {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub authors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LoadConfig {
    #[serde(default)]
    pub lazy: bool,
    #[serde(default)]
    pub hooks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DepsConfig {
    #[serde(default)]
    pub binaries: Vec<String>,
    #[serde(default)]
    pub plugins: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ExportsConfig {
    #[serde(default)]
    pub functions: Vec<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct StateConfig {
    /// Shell command to run for state gathering. Evaled each precmd.
    /// May reference `$PLUGIN_DIR` for the plugin's install directory.
    /// Example: `"my-plugin state"` or `"python3 $PLUGIN_DIR/state.py"`
    #[serde(default)]
    pub gather: Option<String>,
}
