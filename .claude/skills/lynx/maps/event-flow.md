# Event System Flow Map

## Architecture

```
Shell (zsh)                    Rust (lynx-daemon)
──────────────────────         ──────────────────────────────
_lynx_hook_chpwd()
  └── lx event emit ──────────→ IPC: events.sock
       "shell:chpwd"              └── EventBus.dispatch()
       --data "$PWD"                    └── handler1(data)
                                         └── handler2(data)
                                               └── updates SegmentCache
                                                     └── next precmd renders new prompt
```

## Well-Known Event Names (defined in lynx-events/src/types.rs)

```rust
pub const SHELL_CHPWD:        &str = "shell:chpwd";        // cwd changed
pub const SHELL_PREEXEC:      &str = "shell:preexec";      // before command runs
pub const SHELL_PRECMD:       &str = "shell:precmd";       // before prompt renders
pub const SHELL_CONTEXT_CHANGED: &str = "shell:context-changed";
pub const CONFIG_CHANGED:     &str = "config:changed";
pub const PLUGIN_LOADED:      &str = "plugin:loaded";
pub const PLUGIN_FAILED:      &str = "plugin:failed";
pub const THEME_CHANGED:      &str = "theme:changed";
pub const GIT_STATE_UPDATED:  &str = "git:state-updated";  // git plugin fires this
pub const TASK_COMPLETED:     &str = "task:completed";
pub const TASK_FAILED:        &str = "task:failed";
```

## Plugin Registration

Plugins subscribe via plugin.toml hooks[]:
```toml
[load]
hooks = ["chpwd", "precmd"]    # maps to shell:chpwd, shell:precmd
```

The ACTIVATE lifecycle stage registers these with EventBus.

## Cross-Plugin Communication Rule

Plugins NEVER call each other's functions directly.
Plugin A needs data from Plugin B → Plugin B fires an event → Plugin A subscribes.

Example: prompt needs git state
- WRONG: prompt calls git_branch() directly
- CORRECT: git plugin fires "git:state-updated" → populates SegmentCache → prompt reads cache

## IPC Details

- Socket: $LYNX_RUNTIME_DIR/events.sock (from lynx-core::runtime)
- Protocol: newline-delimited JSON
- Direction: shell → daemon (emit); daemon → shell (callback via lx event trigger)
- Shell side is ALWAYS fire-and-forget (2>/dev/null, non-blocking)
- If daemon is not running: shell-side emit is silently dropped, no error

## Adding a New Event

1. Add constant to lynx-events/src/types.rs
2. Document it in this map
3. Emit it from the correct source
4. Update plugin-lifecycle.md if it affects lifecycle
