# Plugin Authoring Protocol (for agents building plugins)

## Before Writing Any Plugin Code

1. Read `maps/plugin-lifecycle.md` — understand all 4 stages
2. Run `pt check-decision "plugin"` — know the rules
3. Check if the binary dep exists before implementing: `which <binary>`

## Plugin Directory Structure (mandatory)

```
plugins/<name>/
├── plugin.toml          # manifest — required
└── shell/
    ├── init.zsh         # entry point — sources others, under 10 lines
    ├── functions.zsh    # all functions (use _ prefix for internals)
    └── aliases.zsh      # aliases ONLY — never mixed with functions
```

Optional for Rust-backed plugins:
```
    ├── Cargo.toml
    └── src/lib.rs
```

## plugin.toml — All Fields Required

```toml
[plugin]
name        = "my-plugin"      # must match directory name
version     = "0.1.0"          # semver
description = "One line"       # shown in lx plugin list
authors     = ["name"]

[load]
lazy  = false                  # true = defer until first use
hooks = []                     # from well-known list in maps/event-flow.md

[deps]
binaries = []                  # checked at load time — fail fast if missing
plugins  = []                  # other lynx plugins required

[exports]
functions = []                 # ALL public functions — no wildcards
aliases   = []                 # ALL aliases — no wildcards

[contexts]
disabled_in = ["agent", "minimal"]   # REQUIRED if exports.aliases is non-empty
```

## Function Naming Rules

```zsh
# Public (must be in exports.functions)
my_plugin_do_thing() { ... }

# Internal (must NOT be in exports.functions, _ prefix required)
_my_plugin_helper() { ... }
_my_plugin_state=""
```

## Alias Rules

- Aliases MUST be in aliases.zsh — never in functions.zsh or init.zsh
- Aliases MUST have `disabled_in = ["agent", "minimal"]` in plugin.toml
- No alias may shadow a common system command without explicit user opt-in
- All aliases must be listed in exports.aliases

## Segment Cache Protocol

If your plugin feeds data to the prompt:
1. Fire a named event when data changes (e.g. "git:state-updated")
2. Populate SegmentCache with your data in the event handler
3. The prompt segment reads from cache — never from your plugin directly

Never run slow commands (git, kubectl, aws) in the prompt render path.

## init.zsh Template (keep under 10 lines)

```zsh
# <name> plugin — init.zsh
# Sourced by eval-bridge via lx plugin exec <name>

source "${LYNX_PLUGIN_DIR}/shell/functions.zsh"
[[ "$LYNX_CONTEXT" != "agent" && "$LYNX_CONTEXT" != "minimal" ]] && \
  source "${LYNX_PLUGIN_DIR}/shell/aliases.zsh"
```

## Testing a Plugin

```bash
lx plugin add ./plugins/<name>    # validates manifest
lx doctor                          # checks deps and exports
LYNX_CONTEXT=agent lx init        # verify aliases not loaded
LYNX_CONTEXT=interactive lx init  # verify full plugin loads
```
