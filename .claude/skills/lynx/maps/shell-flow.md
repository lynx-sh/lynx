# Shell Integration Flow Map

## Startup Sequence (every new shell)

```
.zshrc
  └── source "${HOME}/.config/lynx/shell/init.zsh"
        └── source shell/core/loader.zsh
              └── eval "$(lx init --context <detected>)"
                    │
                    ├── sets: LYNX_DIR, LYNX_CONTEXT, LYNX_PLUGIN_DIR
                    ├── sources: shell/core/hooks.zsh
                    └── for each eager plugin in load order:
                          eval "$(lx plugin exec <name>)"
                                │
                                └── sets LYNX_PLUGIN_DIR=plugins/<name>/
                                    sources shell/init.zsh
                                    transitions plugin to ACTIVATE state
```

## Lazy Plugin Trigger Sequence

```
User types: gst   (git plugin is lazy)
  └── wrapper function _lynx_lazy_git fires
        └── lx plugin exec git → emits shell glue
              └── eval output → real git functions defined
                    └── _lynx_lazy_git removes itself
                          └── gst called again → real gst runs
```

## Hook → Event Flow

```
zsh precmd fires
  └── _lynx_hook_precmd()
        └── lx event emit "shell:precmd" (fire-and-forget, 2>/dev/null)
              └── IPC → LYNX_RUNTIME_DIR/events.sock
                    └── lynx-daemon EventBus.dispatch("shell:precmd")
                          └── registered handlers (e.g. git plugin cache refresh)
```

## Config Mutation Flow

```
lx <any mutating command>
  ├── snapshot_config()        → ~/.config/lynx/snapshots/<timestamp>/
  ├── validate_new_state()     → error if invalid, no write
  ├── apply_to_disk()          → write config.toml
  └── emit config:changed      → shell reloads affected components
```

## Eval-Bridge Pattern (the ONLY valid pattern for shell integration)

```zsh
# CORRECT: Rust generates zsh, shell evals it
eval "$(lx plugin exec git)"

# WRONG: never do this
source /path/to/rust/output  # not a valid pattern
lx plugin exec git | bash    # never pipe to bash
lx plugin exec git > /tmp/x && source /tmp/x  # no temp files
```

## Shell Layer File Limits

Each file in shell/ must stay under 60 lines. If it grows beyond that:
- Logic that crept in must move to Rust
- The file is doing too many things — split it
