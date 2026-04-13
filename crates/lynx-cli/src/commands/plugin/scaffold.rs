// `lx plugin new` — scaffold a new plugin directory with sensible defaults.
//
// Generates plugin.toml, shell/init.zsh, shell/functions.zsh, shell/aliases.zsh
// with inline comments that explain each field so new contributors don't need
// to read the full protocol docs to get started.

use anyhow::{Result};
use lynx_core::error::LynxError;
use lynx_plugin::namespace::scaffold_convention_comment;
use std::path::PathBuf;

pub(super) fn cmd_new(name: &str) -> Result<()> {
    let dir = PathBuf::from(name);
    if dir.exists() {
        return Err(LynxError::Plugin(format!("directory '{name}' already exists")).into());
    }

    std::fs::create_dir_all(dir.join("shell"))?;

    let toml = format!(
        r#"[plugin]
name        = "{name}"   # unique identifier — must match the directory name
version     = "0.1.0"   # semver; bump when you make breaking changes
description = ""         # shown in `lx plugin list`
authors     = []         # e.g. ["Your Name <you@example.com>"]

[load]
lazy  = false  # true = load only on first use of an exported function
hooks = []     # zsh hooks that trigger load, e.g. ["chpwd", "precmd"]

[deps]
binaries = []  # required binaries, e.g. ["git", "fzf"] — checked at load
plugins  = []  # other lynx plugins this one depends on

[exports]
# List every function and alias exported to the shell.
# Unlisted names are private — Lynx will refuse to source them.
functions = ["{name}"]   # example: replace with your real function names
aliases   = []           # example: ["g", "gs"] — only loaded in interactive context

[contexts]
# Aliases are never loaded in agent or minimal contexts (D-010).
# Add "interactive" here to also skip functions in non-interactive shells.
disabled_in = ["agent", "minimal"]
"#
    );
    std::fs::write(dir.join(lynx_core::brand::PLUGIN_MANIFEST), toml)?;

    let init_zsh = format!(
        "# {name} — init.zsh  (keep this file under 10 lines)\n\
         # Sources functions and aliases; actual logic lives in functions.zsh.\n\
         source \"${{LYNX_PLUGIN_DIR}}/{name}/shell/functions.zsh\"\n\
         source \"${{LYNX_PLUGIN_DIR}}/{name}/shell/aliases.zsh\"\n",
    );
    std::fs::write(dir.join("shell/init.zsh"), init_zsh)?;

    let functions_zsh = format!(
        "# {name} -- functions.zsh\n\
         # Public functions must match the exports.functions list in plugin.toml.\n\
         # Internal helpers use the _ prefix so Lynx won't export them.\n\
         \n\
         {convention}\n\
         \n\
         # Example public function -- rename and replace with your logic.\n\
         {name}() {{\n\
         {indent}__{name}_run \"$@\"\n\
         }}\n\
         \n\
         # Internal helper -- not exported.\n\
         __{name}_run() {{\n\
         {indent}echo \"{name}: $*\"\n\
         }}\n",
        name = name,
        convention = scaffold_convention_comment(),
        indent = "  ",
    );
    std::fs::write(dir.join("shell/functions.zsh"), functions_zsh)?;

    let aliases_zsh = format!(
        "# {name} — aliases.zsh\n\
         # Aliases are only sourced in interactive context (disabled_in agent+minimal).\n\
         # All aliases must be listed in exports.aliases in plugin.toml.\n\
         \n\
         # Example alias — remove or replace:\n\
         # alias {short}='{name}'\n",
        name = name,
        short = name.chars().next().unwrap_or('x'),
    );
    std::fs::write(dir.join("shell/aliases.zsh"), aliases_zsh)?;

    println!("Created plugin '{name}' at ./{name}/");
    println!();
    println!("  Structure:");
    println!("    {name}/plugin.toml          — manifest (edit exports + deps)");
    println!("    {name}/shell/init.zsh        — entry point (keep under 10 lines)");
    println!("    {name}/shell/functions.zsh   — your functions go here");
    println!("    {name}/shell/aliases.zsh     — aliases (context-gated automatically)");
    println!();
    println!("  Next:");
    println!("    lx plugin add ./{name}       — install and activate");
    println!("    lx plugin list               — verify it's loaded");
    println!("    lx doctor                    — check for issues");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffold_refuses_existing_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let existing = tmp.path().join("existing-plugin");
        std::fs::create_dir_all(&existing).unwrap();

        // Change to temp dir so the relative path resolves
        let _orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();

        let result = cmd_new("existing-plugin");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("already exists"), "unexpected error: {msg}");

        std::env::set_current_dir(_orig).unwrap();
    }

    #[test]
    fn scaffold_creates_all_files() {
        let tmp = tempfile::tempdir().unwrap();
        let _orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();

        let result = cmd_new("test-scaffold");
        assert!(result.is_ok(), "scaffold failed: {:?}", result.err());

        let dir = tmp.path().join("test-scaffold");
        assert!(dir.join(lynx_core::brand::PLUGIN_MANIFEST).exists());
        assert!(dir.join("shell/init.zsh").exists());
        assert!(dir.join("shell/functions.zsh").exists());
        assert!(dir.join("shell/aliases.zsh").exists());

        // Verify manifest is valid TOML
        let content = std::fs::read_to_string(dir.join(lynx_core::brand::PLUGIN_MANIFEST)).unwrap();
        let parsed: toml::Value = toml::from_str(&content).unwrap();
        assert_eq!(parsed["plugin"]["name"].as_str().unwrap(), "test-scaffold");

        std::env::set_current_dir(_orig).unwrap();
    }

    #[test]
    fn scaffold_zsh_files_are_valid_syntax() {
        let tmp = tempfile::tempdir().unwrap();
        let _orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();

        cmd_new("zsh-check").unwrap();

        let dir = tmp.path().join("zsh-check");
        let init = std::fs::read_to_string(dir.join("shell/init.zsh")).unwrap();
        let funcs = std::fs::read_to_string(dir.join("shell/functions.zsh")).unwrap();
        let aliases = std::fs::read_to_string(dir.join("shell/aliases.zsh")).unwrap();

        lynx_test_utils::assert_valid_zsh(&init);
        lynx_test_utils::assert_valid_zsh(&funcs);
        lynx_test_utils::assert_valid_zsh(&aliases);

        std::env::set_current_dir(_orig).unwrap();
    }
}
