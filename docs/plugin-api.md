# Plugin API Reference

This document is the **stable contract** for Lynx plugin authors. Everything
listed here is intentionally stable — changes will be noted in the changelog
and gated behind a deprecation period. Anything not listed here is internal
and may change without notice.

> **See also:** [Plugin Authoring Guide](plugin-authoring.md) for a
> step-by-step tutorial. This document is the reference, not the tutorial.

---

## Table of Contents

1. [Stability Tiers](#stability-tiers)
2. [Environment Variables](#environment-variables)
   - [Stable — read/write by plugins](#stable--readwrite-by-plugins)
   - [Stable — read-only](#stable--read-only)
   - [Unstable / internal](#unstable--internal)
3. [lx Commands](#lx-commands)
   - [Stable plugin-facing commands](#stable-plugin-facing-commands)
   - [Unstable / internal commands](#unstable--internal-commands)
4. [Shell Variable Contracts](#shell-variable-contracts)
5. [Plugin Guard Variable Pattern](#plugin-guard-variable-pattern)
6. [State Cache Contract](#state-cache-contract)
7. [Event Contract](#event-contract)

---

## Stability Tiers

| Tier | Meaning |
|---|---|
| **Stable** | Will not change without a deprecation notice and major version bump. |
| **Unstable** | Internal to Lynx. May change in any release. Do not use from plugins. |

---

## Environment Variables

### Stable — read/write by plugins

These variables are part of the plugin API. Plugins may read and write them.

| Variable | Set by | Purpose |
|---|---|---|
| `LYNX_CONTEXT` | `lx init` | Detected context: `interactive`, `agent`, or `minimal`. Read this to gate context-sensitive behaviour. Never write it. |
| `LYNX_LAST_EXIT_CODE` | precmd hook | Exit code of the last shell command. Read in prompt segments and hooks. |
| `LYNX_BG_JOBS` | precmd hook | Number of background jobs (`${#jobstates}`). |
| `LYNX_VI_MODE` | vi-mode plugin | Current vi-mode indicator (`insert` or `normal`). Write only from a vi-mode plugin. |
| `LYNX_CACHE_<NAME>_STATE` | plugin's hook | JSON-serialized state blob for plugin `<NAME>`. See [State Cache Contract](#state-cache-contract). |

### Stable — read-only

Plugins must not write these. They are exported by Lynx core and are safe to
read.

| Variable | Set by | Purpose |
|---|---|---|
| `LYNX_DIR` | `lx init` | Path to the Lynx installation directory (default: `~/.config/lynx`). Override at shell startup to relocate the install. |
| `LYNX_PLUGIN_DIR` | `lx init` | Path to the installed plugins directory (`$LYNX_DIR/plugins`). |
| `LYNX_THEME` | user / `lx theme set` | Active theme name. Override at shell startup to force a theme. |
| `LYNX_INITIALIZED` | `lx init` | Set after init completes. Used to prevent double-init. Do not read — presence means init ran. |

### Unstable / internal

Do **not** use these from third-party plugins.

| Variable | Reason unstable |
|---|---|
| `LYNX_RUNTIME_DIR` | Runtime socket path — resolved via `runtime_dir()` internally. |
| `LYNX_DAEMON_BIN` | Daemon binary override — for service installers only. |
| `LYNX_LOG` | Internal log verbosity override. |
| `LYNX_SAFE_MODE` | Internal degraded-mode flag. |
| `LYNX_BENCHMARK_MODE` | Set only by `lx benchmark`. |
| `LYNX_LAST_CMD_MS` | Not yet emitted by stable shell layer (reserved). |
| `LYNX_PLUGIN_<NAME>_LOADED` | Load guard — managed by `lx plugin exec/unload`. Do not write. See [Plugin Guard Variable Pattern](#plugin-guard-variable-pattern). |

---

## lx Commands

### Stable plugin-facing commands

These commands are safe to call from plugin shell code. Their output format and
exit codes are stable.

#### `lx event emit <topic> [--data <value>]`

Emit a named event into the Lynx event bus.

```zsh
lx event emit "my-plugin:state-changed" --data "$PWD" 2>/dev/null
```

- Always call with `2>/dev/null` — failures must be silent in plugin hooks.
- `<topic>` should be namespaced: `<plugin-name>:<event>`.
- `--data` is an optional string payload.

#### `lx event on <topic>`

Subscribe to events (used in daemon plugins, not shell glue).

#### `lx plugin exec <name>`

Emit the zsh activation glue for a plugin. Called by the eval-bridge; not
normally called directly from plugin code.

#### `lx plugin unload <name>`

Emit the zsh unload script for a plugin (removes hooks, clears guard var).

#### `lx refresh-state`

Emit zsh that refreshes all enabled plugin state caches. Called by the precmd
hook via `eval "$(lx refresh-state 2>/dev/null)"`. Plugins do not call this
directly.

#### `lx prompt render`

Emit zsh that sets `PROMPT` and `RPROMPT`. Called by the precmd hook. Plugins
do not call this directly, but segments defined in a plugin's Rust crate are
invoked here.

#### `lx context show`

Print the current context (`interactive`, `agent`, or `minimal`). Useful in
plugin init scripts for conditional loading.

```zsh
if [[ "$(lx context show 2>/dev/null)" == "interactive" ]]; then
  # interactive-only setup
fi
```

### Unstable / internal commands

Do **not** call these from third-party plugins.

| Command | Reason unstable |
|---|---|
| `lx init` | Boot strap only — called by `shell/core/loader.zsh` |
| `lx git-state` | Internal state gatherer for the git plugin |
| `lx kubectl-state` | Internal state gatherer for the kubectl plugin |
| `lx prompt render --transient` | ZLE transient-prompt path — interface may change |
| `lx dev *` | Developer utilities — no stability guarantee |
| `lx benchmark *` | Internal profiling |
| `lx migrate *` | Config schema migrations — called by Lynx, not plugins |

---

## Shell Variable Contracts

These zsh variables are written by Lynx core and read by plugin shell code.
They use the `_lynx_` prefix to avoid collisions with user variables.

| Variable | Written by | Contains |
|---|---|---|
| `_lynx_git_state` | `lx git-state` (via refresh-state) | JSON git state blob (branch, dirty, ahead/behind) |
| `_lynx_kubectl_state` | `lx kubectl-state` (via refresh-state) | JSON kubectl state blob (context, namespace) |

**Rules for plugins that define their own state vars:**

- Use the pattern `_lynx_<plugin-name>_state` for your plugin's state variable.
- Populate it in your `chpwd` / `precmd` hook function.
- Read it only from within your own plugin's functions and segments.
- Do not write to another plugin's state variable.

---

## Plugin Guard Variable Pattern

Lynx uses a per-plugin load-guard variable to ensure idempotent loading.

**Format:** `LYNX_PLUGIN_<NAME_UPPERCASE>_LOADED`

Examples:
- plugin `git` → `LYNX_PLUGIN_GIT_LOADED`
- plugin `my-plugin` → `LYNX_PLUGIN_MY_PLUGIN_LOADED`

Lynx manages this variable automatically via `lx plugin exec` and
`lx plugin unload`. Plugin authors **must not** set or unset this variable
manually — doing so will break idempotency guards.

To check whether your plugin is already loaded from within plugin code:

```zsh
# Don't do this — lx plugin exec handles it automatically
[[ -n "$LYNX_PLUGIN_MY_PLUGIN_LOADED" ]] && return 0
```

The guard is only for the internal eval-bridge. External callers should use
`lx plugin list` to check load status.

---

## State Cache Contract

Plugins that gather ambient state (language versions, git status, cloud
contexts) store it as a JSON blob in `LYNX_CACHE_<NAME>_STATE`.

**Rules:**

1. **Name:** `LYNX_CACHE_<PLUGIN_NAME_UPPERCASE_UNDERSCORED>_STATE`
   - `git` → `LYNX_CACHE_GIT_STATE`
   - `my-plugin` → `LYNX_CACHE_MY_PLUGIN_STATE`

2. **Format:** valid JSON object. Recommended minimum schema:
   ```json
   { "version": "1.23.4", "ready": true }
   ```

3. **Lifetime:** export the variable in your `precmd` / `chpwd` hook. Lynx
   does not persist state caches across shell sessions.

4. **Failure:** on failure, set the variable to `""` (empty string) or leave it
   unset. Never export partial/malformed JSON.

5. **Reading:** only your own plugin's Rust segment or shell functions should
   read your plugin's cache var. Do not read another plugin's cache.

---

## Event Contract

Events use a dotted or colon-namespaced topic string.

### Built-in topics emitted by Lynx core

**Shell lifecycle**

| Topic | When | Payload |
|---|---|---|
| `shell:chpwd` | Directory changed | New `$PWD` |
| `shell:preexec` | Before each command | Command string |
| `shell:precmd` | Before each prompt | *(none)* |
| `shell:context-changed` | Context switches (e.g. agent mode activated) | New context string (`interactive` / `agent` / `minimal`) |

**Config and theme**

| Topic | When | Payload |
|---|---|---|
| `config:changed` | Lynx config file modified | *(none)* |
| `theme:changed` | Active theme switched | New theme name |

**Plugin lifecycle**

| Topic | When | Payload |
|---|---|---|
| `plugin:loaded` | A plugin finishes loading | Plugin name |
| `plugin:unloaded` | A plugin is removed/disabled | Plugin name |
| `plugin:failed` | A plugin fails during load or activation | Plugin name |

**Git**

| Topic | When | Payload |
|---|---|---|
| `git:branch-changed` | Current branch changes (detected by state refresh) | New branch name |
| `git:state-updated` | Git cache refreshed (any change to working tree state) | *(none)* |

**Task scheduler**

| Topic | When | Payload |
|---|---|---|
| `task:completed` | A scheduled task finishes successfully | Task name |
| `task:failed` | A scheduled task exits non-zero | Task name |

### Plugin-emitted events

Prefix your events with your plugin name: `<plugin-name>:<event>`.

```zsh
lx event emit "weather:data-fetched" --data "$temp_json" 2>/dev/null
```

### Subscribing in shell code

Event subscription from shell glue is not currently in the stable API.
Daemon plugins may subscribe via `lx event on <topic>`, but the daemon
plugin interface is **unstable** in this release.
