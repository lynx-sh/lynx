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

Read-only keys (via `lx config get` only): `schema_version`.

### Edit Safety

`lx config edit` snapshots the config before opening `$EDITOR`. If the saved file fails
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
