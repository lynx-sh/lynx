# Event System Flow Map

## Architecture

Events are dispatched **in-process** inside each `lx` command invocation.
There is no persistent event bus process. The daemon owns only the task scheduler.

```
lx prompt render  (or: lx event emit, lx plugin exec)
  └── crates/lynx-cli/src/bus.rs :: build_active_bus(context, plugins_dir)
        ├── lifecycle::declare(plugins_dir)       — parse all plugin.toml files
        ├── depgraph::resolve(manifests)           — topo sort, exclude missing bins
        ├── lifecycle::apply_resolve(...)          — mark Resolved/Excluded states
        └── lifecycle::activate(name, manifest, bus)  — register hook handlers
              └── bus.emit("shell:precmd", cwd)
                    └── handlers execute in-process, results written to log
```

Shell hooks set env vars before invoking `lx prompt render`:
```
_lynx_hook_precmd()
  ├── export LYNX_CACHE_GIT_STATE=<json>
  ├── export LYNX_CACHE_KUBECTL_STATE=<json>
  └── eval "$(lx prompt render 2>/dev/null)"   ← runs full lifecycle in-process
```

## Well-Known Event Names (defined in lynx-events/src/types.rs)

Source of truth: `crates/lynx-events/src/types.rs` — always verify there before adding new events.

| Constant | Value | Emitted by |
|---|---|---|
| `SHELL_CHPWD` | `shell:chpwd` | shell hook |
| `SHELL_PREEXEC` | `shell:preexec` | shell hook |
| `SHELL_PRECMD` | `shell:precmd` | `lx prompt render` (in-process) |
| `SHELL_CONTEXT_CHANGED` | `shell:context-changed` | `lx context` |
| `CONFIG_CHANGED` | `config:changed` | config mutators |
| `THEME_CHANGED` | `theme:changed` | `lx theme` |
| `PLUGIN_LOADED` | `plugin:loaded` | `lx plugin exec` (in-process) |
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

`lifecycle::activate()` registers these handlers on the in-process EventBus
during `build_active_bus()`. Handlers run and the process exits — no persistence needed.

## Cross-Plugin Communication Rule

Plugins NEVER call each other's functions directly.
Plugin A needs data from Plugin B → Plugin B fires an event → Plugin A subscribes.

Example: prompt needs git state
- WRONG: prompt calls git_branch() directly
- CORRECT: git plugin fires "git:state-updated" → populates SegmentCache → prompt reads cache

## No Daemon IPC for Events

The daemon does NOT own the EventBus. There is no events.sock.
`lx event emit <name>` runs the full plugin lifecycle in-process and fires the event.
`lx event on` does not exist — persistent subscriptions are handled by `add-zsh-hook`
via plugin manifests, which is already wired by `lx plugin exec`.

## Adding a New Event

1. Add constant to lynx-events/src/types.rs
2. Document it in this map
3. Emit it from the correct lx command (in-process via bus::build_active_bus)
4. Update plugin-lifecycle.md if it affects lifecycle
