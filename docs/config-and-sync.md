# Config & Sync

Reference for `lx config` and `lx sync` — inspecting, editing, and syncing your Lynx
configuration across machines.

## lx config

Lynx config lives at `~/.config/lynx/config.toml`. All subcommands operate on this file.

| Subcommand                        | Description                                               |
|-----------------------------------|-----------------------------------------------------------|
| `lx config show`                  | Print current config with secrets redacted                |
| `lx config edit`                  | Open config in `$EDITOR`; validates and rolls back on error |
| `lx config validate`              | Validate TOML syntax and schema; reports line numbers     |
| `lx config get <key>`             | Print a single value by dot-notation key                  |
| `lx config set <key> <value>`     | Update a value (snapshot → validate → apply)              |
| `lx config examples`              | Show real-world usage examples                            |

### Settable Keys

| Key              | Type                                  | Example                         |
|------------------|---------------------------------------|---------------------------------|
| `active_theme`   | Theme name (must exist in `themes/`)  | `lx config set active_theme catppuccin` |
| `active_context` | `interactive` \| `agent` \| `minimal` | `lx config set active_context agent` |
| `sync.remote`    | Git remote URL (or empty to clear)    | `lx config set sync.remote git@github.com:you/dotfiles.git` |
| `tui.enabled`    | `true` \| `false` (default: `true`)   | `lx config set tui.enabled false` |
| `editor`         | Editor binary name (any editor)       | `lx config set editor code` |

Read-only keys (via `lx config get` only): `schema_version`, `onboarding_complete`.

### User Aliases

User-defined aliases are stored in `config.toml` and managed via `lx alias`:

```bash
lx alias add gs "git status"              # interactive context only (default)
lx alias add ll "ls -la" --all-contexts   # all non-agent/minimal contexts
lx alias list                             # TUI view (user + plugin aliases merged)
lx alias remove gs
```

`lx alias add` takes effect immediately in the current shell (eval bridge). Aliases
persist across sessions via the init script — they are never loaded in `agent` or
`minimal` contexts (D-010).

TOML representation:

```toml
[[aliases]]
name = "gs"
command = "git status"
description = "quick git status"   # optional
context = "interactive"            # "interactive" (default) or "all"
```

### User PATH Entries

User-managed PATH entries are stored in config and prepended to `$PATH` at shell init:

```bash
lx path add /usr/local/sbin --label "sbin"   # label is optional
lx path list
lx path remove /usr/local/sbin
```

PATH entries take effect on the next shell start. They are always emitted regardless
of context (agent shells may still need custom paths).

TOML representation:

```toml
[[paths]]
path = "/usr/local/sbin"
label = "sbin"   # optional, shown in lx path list
```

### TUI Output Mode

By default, list commands (`lx theme list`, `lx plugin list`, `lx run list`, etc.) and
workflow execution use an interactive ratatui TUI. You can disable this at three levels:

**Config** — permanent, affects all sessions:
```toml
# ~/.config/lynx/config.toml
[tui]
enabled = false
```

**Environment variable** — per-session or per-command:
```bash
LYNX_NO_TUI=1 lx theme list   # plain text for this command only
export LYNX_NO_TUI=1           # plain text for the whole session
```

**Automatic** — TUI is always disabled when:
- stdout is not a TTY (pipe, redirect)
- `LYNX_CONTEXT=agent` is set
- `CLAUDECODE` or `CURSOR_CLI` is set (AI agent terminals)
- `CI=true` or `CI=1` is set

When TUI is disabled, all commands fall back to structured plain-text output suitable
for scripts, CI pipelines, and AI agents.

### Editor

Set your preferred editor once in config and Lynx exports it as `$VISUAL` at shell init:

```bash
lx config set editor code    # VS Code
lx config set editor zed     # Zed
lx config set editor vim     # Vim
lx config set editor nano    # Nano
```

Any editor binary works. If `$VISUAL` is already set in your environment, that takes precedence
over the config value. Unset `editor` to rely entirely on `$VISUAL`/`$EDITOR` from your shell.

### Edit Safety

`lx config edit` snapshots the config before opening your editor. If the saved file fails
TOML parse or schema validation, Lynx automatically restores the snapshot and reports the
error — your previous config is never lost.

```bash
lx config show                          # inspect current state
lx config validate                      # check before a manual edit
lx config get active_theme              # print a single value
lx config set active_theme nord         # apply a change safely
lx config edit                          # open in $EDITOR with auto-rollback
```

---

## lx sync

`lx sync` provides git-backed config sync. Your `~/.config/lynx/` directory becomes a git
repo that you push and pull to keep multiple machines in sync.

| Subcommand               | Description                                               |
|--------------------------|-----------------------------------------------------------|
| `lx sync init <remote>`  | Init git repo in config dir and set the remote            |
| `lx sync push`           | Stage TOML files, commit with timestamp, push to remote   |
| `lx sync pull`           | Fetch and fast-forward merge from remote                  |
| `lx sync status`         | Show ahead/behind commit counts vs. remote                |

### Workflow

```bash
# First machine
lx sync init git@github.com:you/lynx-config.git
lx sync push

# Second machine (after cloning or fresh install)
lx sync init git@github.com:you/lynx-config.git
lx sync pull

# Daily use
lx sync status          # check if out of sync
lx sync push            # after local changes
lx sync pull            # before starting a new session
```

### What Gets Synced

`lx sync push` stages all `*.toml` files and `.gitignore` in the config dir. A `.gitignore`
is auto-created on `lx sync init` that excludes:

```
snapshots/
benchmarks.jsonl
.update-check
*.env
*secret*
*secrets*
*credentials*
*private*
```

Secrets and ephemeral data are never committed. Only TOML config files cross the wire.
