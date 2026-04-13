# Lynx

A fast, Rust-powered zsh shell framework. Context-aware, plugin-isolated, theme-driven.

```
~/code/lynx   main +  ↻ 1            2.3s AI
$
```

---

## Why Lynx

| | Lynx | Oh-My-Zsh | Starship | Prezto |
|---|---|---|---|---|
| Language | Rust + thin zsh | Pure zsh | Rust | Pure zsh |
| Plugin isolation | Manifest-declared exports | None | N/A | None |
| Agent-context aware | Automatic | No | No | No |
| Config format | TOML | zsh code | TOML | zsh code |
| Workflow engine + cron | Built-in | No | No | No |
| Web dashboard | Built-in | No | No | No |
| Prompt segments | Concurrent (tokio) | Sequential | Concurrent | Sequential |
| Config sync | Git-backed | Manual | Manual | Manual |

**Lynx is for you if:**
- You want a framework that gets out of the way in AI agent sessions
- You write plugins that need clean isolation and declared exports
- You want config you can diff, sync, and roll back

**Lynx is not for you if:**
- You need to reuse an existing OMZ plugin library (no compatibility layer)
- You want a framework without a build step (Lynx requires `cargo`)

---

## Install

```bash
curl -sf https://raw.githubusercontent.com/lynx-sh/lynx/main/install.sh | sh
```

The installer:
1. Builds `lx` from source (requires Rust toolchain — installs if missing)
2. Copies `lx` to `~/.local/bin`
3. Copies the shell integration layer to `~/.config/lynx/`
4. Launches `lx onboard` — an interactive TUI wizard to pick your theme, enable plugins, and wire up your shell

Verify:
```bash
lx --version
lx doctor
```

To re-run the setup wizard at any time:
```bash
lx onboard          # re-open the wizard
lx onboard --force  # re-run even if already completed
```

### Manual install

```bash
git clone https://github.com/lynx-sh/lynx.git
cd lynx
cargo build --release
cp target/release/lx ~/.local/bin/
lx setup --source .   # copies shell/, themes/, writes config.toml
lx onboard            # interactive setup wizard
```

---

## 5-Minute Quickstart

### 1. Check your setup

```bash
lx doctor     # diagnoses any issues with your install
```

### 2. Pick a theme

```bash
lx theme list           # browse themes (interactive TUI)
lx theme set tokyo-night # switch to a theme
lx theme convert <url>  # import an OMZ theme
```

### 3. Add a plugin

```bash
lx plugin add git        # install the git integration plugin
lx plugin list           # confirm it's loaded
```

### 4. Switch context

```bash
lx context set agent     # simulate agent context (aliases unloaded)
lx context set interactive
```

### 5. Run a workflow

```bash
lx run deploy env=staging     # execute a workflow with params
lx run list                   # see available workflows
lx jobs list                  # check job status
```

### 6. Schedule a cron task

```bash
lx cron add backup --run "tar czf ~/backup.tar.gz ~/code" --cron "0 2 * * *"
lx cron list
```

### 7. Open the dashboard

```bash
lx dashboard                  # full web UI for managing everything
```

---

## Features

### Context-aware loading

Lynx automatically detects when it's running inside an AI agent, CI, or minimal
environment and adjusts what loads:

- **Interactive** — full plugin suite, aliases, colorized prompt
- **Agent** — plugins load without aliases; minimal prompt; no interference with agent commands
- **Minimal** — only `dir` segment; no plugins

Detection is automatic with canonical env vars:
- `CLAUDECODE=1` or `CURSOR_CLI=<id>` -> `agent`
- `CI=true` -> `minimal`
- otherwise -> `interactive`

`LYNX_CONTEXT` can explicitly override detection (`interactive|agent|minimal`) when needed.

### Plugin isolation

Every plugin declares exactly what it exports in `plugin.toml`:

```toml
[exports]
functions = ["git_branch", "git_dirty"]
aliases   = ["g", "gs"]
```

Lynx refuses to source anything not in this list. `lx doctor` warns on any
namespace violations.

### Theme system

Themes are TOML files. Segments are evaluated concurrently. Switch themes
instantly without restarting your shell:

```bash
lx theme set powerline
```

### Workflow engine

Define multi-step workflows as TOML files with typed params, concurrent groups,
9 built-in runners (sh, bash, python, node, go, cargo, curl, docker, zsh), and
retry/timeout/condition logic:

```bash
lx run deploy env=production  # execute with params
lx run deploy --dry-run       # preview steps without running
lx jobs list                  # view running and completed jobs
```

### Cron scheduler

Run commands on a schedule, in the background, with persistent logs:

```bash
lx cron add sync --run "lx sync push" --cron "*/30 * * * *"
lx cron logs sync
```

### Dashboard

Full web UI for managing themes, plugins, registry, workflows, cron, intros,
and system config. Starts a local server, opens your browser:

```bash
lx dashboard
```

### Config sync

Sync your config across machines via git:

```bash
lx sync init git@github.com:you/lynx-config.git
lx sync push
lx sync pull    # on another machine
```

### Rollback

Every config change is snapshotted. Roll back to any point:

```bash
lx rollback
lx rollback --last
```

---

## Plugin Ecosystem

First-party plugins (included in the repo):

| Plugin | What it does |
|---|---|
| `git` | Branch, dirty status, ahead/behind cache for prompt segments |
| `fzf` | `ctrl-r` history search, `ctrl-t` file picker |
| `atuin` | Atuin shell history integration |
| `kubectl` | Kubectl context/namespace cache for prompt segment, `kctx`/`kns` switchers |

Install any plugin:
```bash
lx plugin add git
lx plugin add fzf
```

Build your own: see [Plugin Authoring Guide](docs/plugin-authoring.md).

---

## Architecture

All logic lives in Rust. The shell layer is ~200 lines of thin glue:

```
~/.zshrc → source shell/init.zsh → eval "$(lx init)" → shell ready
                                                  ↳ eval "$(lx plugin exec git)"
                                                  ↳ eval "$(lx plugin exec fzf)"

Each prompt: eval "$(lx prompt render)"   ← sets PROMPT + RPROMPT
```

See [Architecture doc](docs/architecture.md) for the full crate dependency map,
plugin lifecycle, and event system.

---

## Documentation

- [Architecture](docs/architecture.md) — crate map, shell flow, event system
- [Workflows](docs/workflows.md) — TOML workflow schema, runners, `lx run`
- [Dashboard](docs/dashboard.md) — web UI architecture and endpoints
- [Plugin Authoring](docs/plugin-authoring.md) — build your own plugin
- [Theme Authoring](docs/theme-authoring.md) — build your own theme
- [Contributing](CONTRIBUTING.md) — dev setup, PR process, plugin submission

---

## Status

Lynx is in active development. Core features are working:

| Feature | Status |
|---|---|
| Shell init and eval-bridge | ✓ Stable |
| Plugin load/unload/lifecycle | ✓ Stable |
| Theme system and segments | ✓ Stable |
| Context detection | ✓ Stable |
| Plugin registry (fetch/verify) | ✓ Stable |
| Profile system | ✓ Stable |
| Cron scheduler | ✓ Stable |
| Workflow engine | ✓ Stable |
| Dashboard (web UI) | ✓ Stable |
| Config sync | ✓ Stable |
| Prompt rendering (concurrent) | ✓ Stable |
| Custom segment API | ✓ Stable (Rust contributors) |
| Plugin hot-reload | Planned |
| Windows (WSL only) | Planned |

---

## License

MIT
