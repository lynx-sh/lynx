# Lynx Architecture

This document covers the structural decisions that shape Lynx's codebase. For
*why* each decision was made, run `pt decisions` — that is the single source of truth.

---

## Crate Dependency Graph

Dependencies flow strictly downward. Sideways imports between crates at the
same level are forbidden (P0 violation). `lynx-cli` is the only assembler —
it depends on everything but implements nothing.

```
lynx-core          (foundation: types, error, runtime paths — no internal deps)
├── lynx-config    (user config TOML — reads/writes ~/.config/lynx/config.toml)
├── lynx-manifest  (plugin.toml parser and validator)
├── lynx-events    (async event bus, IPC socket protocol)
├── lynx-template  (token substitution engine — used by shell glue generators)
└── lynx-shell     (zsh glue generator — thin shell script builders)
    └── lynx-depgraph    (plugin dep-graph resolver, lifecycle orchestrator)
        └── lynx-prompt    (segment evaluation, concurrent rendering)
            └── lynx-theme     (theme TOML loader, color engine, terminal capability)

lynx-plugin        (exec script gen, lazy wrappers, namespace lint, context filter)
lynx-task          (task parser + scheduler runtime primitives)
lynx-daemon        (background process hosting event dispatch + task scheduler loops)
lynx-registry      (plugin index fetch, checksum verify, version lock)
lynx-convert       (OMZ theme converter)
lynx-intro         (startup intro renderer — ASCII art, system info)
lynx-doctor        (health check library — environment diagnostics)
lynx-workflow      (TOML workflow schema, runners, step executor, job manager)
lynx-dashboard     (local web UI — Axum server, embedded HTML/CSS/JS frontend)
lynx-test-utils    (dev-dependency only — fixtures, temp HOME, zsh validators)

lynx-cli           (lx binary — assembles all crates, dispatches subcommands)
```

**Hard rules enforced by convention and CI:**
- `lynx-core` depends on nothing internal
- `lynx-prompt` cannot depend on `lynx-depgraph` (circular)
- `lynx-events` cannot depend on `lynx-plugin` (circular)
- Nothing depends on `lynx-cli`

---

## Shell Integration Flow

Lynx uses an *eval-bridge* pattern. The `lx` binary never sources shell scripts
directly — it prints zsh to stdout, and the shell evals it. This keeps all logic
in Rust and keeps the shell layer thin and testable.

```
~/.zshrc
  └── source ~/.config/lynx/shell/init.zsh
        └── source shell/core/loader.zsh
              └── eval "$(lx init)"
                    ├── exports LYNX_DIR, LYNX_CONTEXT, LYNX_PLUGIN_DIR
                    ├── sources shell/core/hooks.zsh    (zsh hook bridge)
                    └── for each enabled plugin:
                          eval "$(lx plugin exec <name>)"
                                ├── sources plugin/shell/init.zsh
                                └── registers hooks: add-zsh-hook <hook> _<name>_plugin_<hook>
```

**Shell layer constraints** (enforced, violations are P0):
- Each file in `shell/` must be under 60 lines
- No conditional logic in shell files — logic lives in Rust
- All `lx` calls use `2>/dev/null` — failures are always silent
- Never source Rust output with `source` — always use `eval "$(...)"` 

---

## Plugin Lifecycle

Every plugin passes through four stages before its functions are available in
the shell. The lifecycle is orchestrated by `lynx-depgraph`.

```
DECLARE → RESOLVE → LOAD → ACTIVATE

DECLARE:   Parse all plugin.toml manifests from enabled_plugins in config.
           Validates schema version and required fields.

RESOLVE:   Topological sort by [deps].plugins. Apply context filter:
           plugins with "interactive" in [contexts].disabled_in are skipped
           when LYNX_CONTEXT=agent or LYNX_CONTEXT=minimal.
           Binary deps are checked here — missing dep = plugin skipped, not error.

LOAD:      Eager plugins (lazy=false): eval "$(lx plugin exec <name>)" now.
           Lazy plugins (lazy=true): register a one-shot trigger that sources
           on first invocation of any exported function.

ACTIVATE:  For each hook in [load].hooks:
           eval "add-zsh-hook <hook> _<pluginname>_plugin_<hook>"
           The hook function convention is _<name>_plugin_<hookname>().
           Idempotency guard: LYNX_PLUGIN_<NAME>_LOADED prevents double-load.
```

---

## Event System

Events are dispatched in-process inside each `lx` command invocation.
`lx` instantiates an `EventBus`, runs the full plugin lifecycle
(declare → resolve → activate) to register handlers, emits the event,
and exits. No daemon required for event dispatch.

```
lx prompt render (or lx event emit, lx plugin exec)
  └── bus::build_active_bus()
        └── lifecycle::declare() + resolve() + activate()
              └── plugin handlers registered on in-process EventBus
                    └── bus.emit("shell:precmd", ...)
                          └── handlers execute, results observable via lx event log
```

The daemon owns only the task scheduler. `lx event emit` can also be
used from the shell directly to fire events for debugging.

The precmd hook also runs prompt rendering synchronously before the event:

```
_lynx_hook_precmd()
  ├── export LYNX_CACHE_GIT_STATE=<json from _lynx_git_state assoc array>
  ├── export LYNX_CACHE_KUBECTL_STATE=<json from _lynx_kubectl_state assoc array>
  ├── eval "$(lx prompt render 2>/dev/null)"   ← sets PROMPT and RPROMPT
  └── lx event emit "shell:precmd"
```

---

## Prompt Rendering

Prompt segments are evaluated concurrently via `tokio::join`. No segment may
perform blocking I/O — slow data (git state, kubectl context) must come from
the cache, which is populated by plugin hook functions before `lx prompt render`
is called.

```
lx prompt render
  ├── reads LYNX_CACHE_GIT_STATE env var     → cache["git_state"]
  ├── reads LYNX_CACHE_KUBECTL_STATE env var → cache["kubectl_state"]
  ├── reads LynxConfig.active_profile        → cache["profile_state"]
  ├── loads active theme (LYNX_THEME env var → config.active_theme → brand::DEFAULT_THEME fallback)
  ├── evaluates all segments concurrently (tokio)
  └── prints: PROMPT="..." \n RPROMPT="..."   ← eval'd by precmd
```

---

## Config Mutation Protocol

Every command that mutates config must follow this sequence. Skipping any step
is a P0 violation (D-007).

```
lx <mutating command>
  ├── snapshot current config to ~/.config/lynx/snapshots/<timestamp>.toml
  ├── validate proposed new state
  ├── apply to disk
  └── emit config:changed event → shell reloads affected components
```

The rollback command (`lx rollback`) lists and restores snapshots.

---

## Context System

Lynx operates in one of three contexts, detected automatically at init time.

| Context | Detected when | Effect |
|---|---|---|
| `interactive` | Default — normal terminal session | All plugins and aliases load |
| `agent` | `CLAUDECODE=1` or `CURSOR_CLI=<value>` | Aliases skipped; minimal prompt |
| `minimal` | `CI=true` | Only essential plugins load |

Detection is automatic by default. `LYNX_CONTEXT` may be set explicitly to
`interactive`, `agent`, or `minimal` and takes precedence over auto-detection.
Plugins declare
`disabled_in = ["agent", "minimal"]` in `[contexts]` to opt out in non-interactive shells.

---

## Invariant Guardrail System

`scripts/verify-guardrails.sh` is the offline conformance verifier. It runs 38 checks
across 5 architectural drift classes and must pass before every PR merge.

| Class | What drifts | Where enforced |
|---|---|---|
| Shell protocol | Static shell files grow > line limit or gain branching logic | `test_init.bats` |
| Context mismatch | `CLAUDECODE`/`CURSOR_CLI`/`CI` constants removed from detector | `test_context.bats` |
| Dep map drift | Forbidden crate dep pairs introduced (circular or upward) | `test_doctor.bats` |
| Checksum enforcement | `validate_index` removed from `fetch_plugin` pipeline | `test_doctor.bats` |
| Docs-command mismatch | CLI subcommands added without README docs | `test_doctor.bats` |

CI runs these via `.github/workflows/guardrails.yml` on every push and PR.

Binary dependency guards (e.g. `command -v kubectl`) are generated by
`lynx-plugin::exec::generate_exec_script` from `deps.binaries` in `plugin.toml`.
They appear in the eval'd output — never in static `shell/init.zsh` files.

---

## Data Flows at a Glance

| Operation | Entry point | Key crates | Output |
|---|---|---|---|
| Shell init | `.zshrc` sources `init.zsh` | lynx-cli, lynx-depgraph, lynx-plugin | zsh eval'd in shell |
| Plugin load | `lx plugin exec <name>` | lynx-plugin, lynx-manifest | zsh eval'd in shell |
| Prompt render | `lx prompt render` | lynx-prompt, lynx-theme | `PROMPT=` / `RPROMPT=` assignments |
| Config change | `lx config set <k> <v>` | lynx-config, lynx-core | TOML on disk + event emitted |
| Plugin install | `lx plugin add <name>` | lynx-registry, lynx-manifest | plugin dir + config update |
| Profile switch | `lx profile switch <name>` | lynx-config, lynx-cli | config update + plugin diff eval'd |
