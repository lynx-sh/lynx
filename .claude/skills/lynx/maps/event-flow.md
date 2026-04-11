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

Source of truth: `crates/lynx-events/src/types.rs` — always verify there before adding new events.

| Constant | Value | Emitted by |
|---|---|---|
| `SHELL_CHPWD` | `shell:chpwd` | shell hook |
| `SHELL_PREEXEC` | `shell:preexec` | shell hook |
| `SHELL_PRECMD` | `shell:precmd` | shell hook |
| `SHELL_CONTEXT_CHANGED` | `shell:context-changed` | `lx context` |
| `CONFIG_CHANGED` | `config:changed` | config mutators |
| `THEME_CHANGED` | `theme:changed` | `lx theme` |
| `PLUGIN_LOADED` | `plugin:loaded` | plugin lifecycle |
| `PLUGIN_UNLOADED` | `plugin:unloaded` | plugin lifecycle |
| `PLUGIN_FAILED` | `plugin:failed` | plugin lifecycle |
| `GIT_BRANCH_CHANGED` | `git:branch-changed` | git plugin |
| `GIT_STATE_UPDATED` | `git:state-updated` | git plugin (planned) |
| `TASK_COMPLETED` | `task:completed` | task scheduler (planned) |
| `TASK_FAILED` | `task:failed` | task scheduler (planned) |

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
