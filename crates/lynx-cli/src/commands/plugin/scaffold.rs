// `lx plugin new` — scaffold a new plugin directory with sensible defaults.
//
// Generates plugin.toml, shell/init.zsh, shell/functions.zsh, shell/aliases.zsh
// with inline comments that explain each field so new contributors don't need
// to read the full protocol docs to get started.

use anyhow::{bail, Result};
use lynx_plugin::namespace::scaffold_convention_comment;
use std::path::PathBuf;

pub(super) async fn cmd_new(name: &str) -> Result<()> {
    let dir = PathBuf::from(name);
    if dir.exists() {
        bail!("directory '{}' already exists.", name);
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
"#,
        name = name
    );
    std::fs::write(dir.join(lynx_core::brand::PLUGIN_MANIFEST), toml)?;

    let init_zsh = format!(
        "# {name} — init.zsh  (keep this file under 10 lines)\n\
         # Sources functions and aliases; actual logic lives in functions.zsh.\n\
         source \"${{LYNX_PLUGIN_DIR}}/{name}/shell/functions.zsh\"\n\
         source \"${{LYNX_PLUGIN_DIR}}/{name}/shell/aliases.zsh\"\n",
        name = name,
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

    println!("Created plugin '{}' at ./{}/", name, name);
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
