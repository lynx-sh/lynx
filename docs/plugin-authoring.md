# Plugin Authoring Guide

Lynx plugins are self-contained directories with a `plugin.toml` manifest,
a thin zsh shell layer, and any Rust prompt segments they need. This guide
walks you through building a real plugin from scratch.

---

## Table of Contents

1. [Concepts](#concepts)
2. [Directory Structure](#directory-structure)
3. [plugin.toml Reference](#plugintoml-reference)
4. [Shell Layer](#shell-layer)
5. [The Four Lifecycle Stages](#the-four-lifecycle-stages)
6. [Hooks: Responding to Shell Events](#hooks-responding-to-shell-events)
7. [Exports and Namespace Isolation](#exports-and-namespace-isolation)
8. [Context Gating](#context-gating)
9. [Adding a Prompt Segment](#adding-a-prompt-segment)
10. [Testing Your Plugin](#testing-your-plugin)
11. [Worked Example: weather plugin](#worked-example-weather-plugin)
12. [Submitting to the Registry](#submitting-to-the-registry)

---

## Concepts

A Lynx plugin is a directory that Lynx loads into the shell at init time (or
lazily on first use). Plugins declare exactly what they export — Lynx refuses
to source anything not listed in `[exports]`. This makes plugins auditable and
prevents name collisions.

The shell layer is intentionally thin: it sources functions and aliases. Heavy
logic — caching, state computation, prompt rendering — should live in zsh
functions (for shell-speed tasks) or in a Rust segment (for prompt rendering).

---

## Directory Structure

```
my-plugin/
├── plugin.toml          ← manifest (required)
└── shell/
    ├── init.zsh         ← entry point (required, must be ≤10 lines)
    ├── functions.zsh    ← public and internal functions
    └── aliases.zsh      ← aliases (only loaded in interactive context)
```

Optional additions:
```
my-plugin/
├── tests/
│   └── test_my_plugin.bats   ← bats integration tests
```

Scaffold a new plugin with:

```bash
lx plugin new my-plugin
cd my-plugin
```

---

## plugin.toml Reference

```toml
[plugin]
name        = "my-plugin"    # unique ID — must match directory name exactly
version     = "0.1.0"        # semver; bump on breaking changes
description = "Does X"       # shown in lx plugin search and lx plugin list
authors     = ["Your Name <you@example.com>"]

[load]
lazy  = false    # true = defer load until a listed export is first called
hooks = []       # zsh hooks that trigger state refresh, e.g. ["chpwd", "precmd"]

[deps]
binaries = []    # required binaries checked at load time, e.g. ["git", "kubectl"]
plugins  = []    # other Lynx plugins this one requires, e.g. ["git"]

[exports]
# IMPORTANT: list every symbol you expose. Unlisted names are private.
# Lynx will refuse to source functions not in this list.
functions = ["my_func"]       # public functions
aliases   = ["mf"]            # aliases — must also be in [contexts].disabled_in

[contexts]
# Aliases must NEVER load in agent or minimal context (rule D-010).
# Add "interactive" to skip the entire plugin in non-interactive shells.
disabled_in = ["agent", "minimal"]
```

### Field details

**`[load].lazy`** — When `true`, Lynx registers a stub function for each
exported function. On first call, the stub sources `shell/init.zsh`, removes
itself, and replays the original call. Use lazy for plugins with heavy deps
(e.g., `kubectl`) where you don't want init-time overhead.

**`[load].hooks`** — List zsh hook names. Lynx will call
`add-zsh-hook <hook> _<pluginname>_plugin_<hook>` after sourcing `init.zsh`.
Your `functions.zsh` must define `_<pluginname>_plugin_<hookname>()`.

**`[deps].binaries`** — Lynx checks these with `command -v` before loading. If
any are missing, the plugin is skipped with a diagnostic (not an error). This
is better than loading and failing mysteriously.

**`[exports].functions`** — Every public function your plugin exposes. Internal
helpers must use the `__<pluginname>_` prefix convention (double underscore,
plugin name prefix) — they are not exported and will not be checked.

**`[exports].aliases`** — Every alias. These are only sourced when the context
is not in `[contexts].disabled_in`.

---

## Shell Layer

### init.zsh

The entry point. Must be ≤10 lines. Sources your other files. Never put logic here.

```zsh
# my-plugin — init.zsh
source "${LYNX_PLUGIN_DIR}/my-plugin/shell/functions.zsh"
source "${LYNX_PLUGIN_DIR}/my-plugin/shell/aliases.zsh"
```

`LYNX_PLUGIN_DIR` is set by Lynx before sourcing `init.zsh`.

### functions.zsh

Public functions are named exactly as declared in `[exports].functions`.
Internal helpers use the `__<pluginname>_<name>` convention.

```zsh
# my-plugin — functions.zsh

# Public function — matches exports.functions in plugin.toml
my_func() {
  __my_plugin_do_work "$@"
}

# Internal helper — not exported, safe to use freely
__my_plugin_do_work() {
  echo "doing: $*"
}
```

### aliases.zsh

Aliases only. No functions, no logic.

```zsh
# my-plugin — aliases.zsh
# Loaded only in interactive context (disabled_in agent + minimal).
alias mf='my_func'
```

---

## The Four Lifecycle Stages

Understanding the lifecycle helps you predict when your code runs.

```
DECLARE → RESOLVE → LOAD → ACTIVATE
```

**DECLARE** — Lynx reads your `plugin.toml`. This happens on every shell init.
Your zsh files are not sourced yet. Errors here prevent loading (schema
validation failures are surfaced by `lx doctor`).

**RESOLVE** — Lynx sorts plugins topologically by `[deps].plugins` and checks
`[deps].binaries`. If a binary is missing, your plugin is skipped. Context
filter runs here: if your plugin is in `disabled_in` for the active context,
it is not loaded.

**LOAD** — For eager plugins (`lazy=false`): Lynx runs `eval "$(lx plugin exec
<name>)"` which sources your `init.zsh`. For lazy plugins: stubs are registered.

**ACTIVATE** — Lynx registers your hook functions via `add-zsh-hook`. Your
plugin is now fully active.

---

## Hooks: Responding to Shell Events

Hooks let your plugin update state when the user navigates or runs a command.
The most common use case is keeping a state cache fresh for prompt rendering.

### Declaring hooks

In `plugin.toml`:
```toml
[load]
hooks = ["chpwd", "precmd"]
```

### Implementing hook functions

In `functions.zsh`, define `_<pluginname>_plugin_<hookname>()`:

```zsh
typeset -gA _my_plugin_state    # global assoc array for state

_my_plugin_chpwd() {
  __my_plugin_refresh_state
}

_my_plugin_precmd() {
  __my_plugin_refresh_state
}

__my_plugin_refresh_state() {
  # Populate state. This runs on every prompt — keep it fast.
  _my_plugin_state=(key "value")
}
```

The naming convention `_<pluginname>_plugin_<hookname>` is required — Lynx
derives the function name from your manifest automatically.

### Available hooks

| Hook | Triggers when |
|---|---|
| `chpwd` | User changes directory |
| `precmd` | Before each prompt is drawn |
| `preexec` | Before each command runs (receives the command string as `$1`) |

---

## Exports and Namespace Isolation

Every function in your `[exports].functions` list must follow one rule:
**the function name must be unique and not clash with builtins or other plugins**.

Lynx enforces a lint: if you source a function that is not in your exports list,
it warns during `lx doctor`. This catches accidental pollution.

Internal helpers must use the `__<pluginname>_` prefix:

```zsh
# Good — internal, uses prefix
__weather_fetch() { ... }

# Bad — will be flagged by lx doctor
_fetch_weather() { ... }   # no plugin prefix
```

Lynx does not enforce strict sandboxing at the zsh level (that would require
subshells and break performance). The convention + lint is the isolation mechanism.

---

## Context Gating

Contexts tell Lynx what environment the shell is running in:

| Context | When active |
|---|---|
| `interactive` | Normal terminal session |
| `agent` | `CLAUDECODE=1` or `CURSOR_CLI=<value>` |
| `minimal` | `CI=true` |

Your plugin's `[contexts].disabled_in` controls which contexts skip loading.
**Aliases must always list `"agent"` and `"minimal"`** — aliases in agent context
shadow commands and break agent behavior.

```toml
[contexts]
disabled_in = ["agent", "minimal"]   # aliases must list both
```

To disable the entire plugin (not just aliases) in agent context, add your
plugin's functions to `disabled_in` as well — but this is usually wrong. Most
plugins should load in agent context without aliases.

---

## Adding a Prompt Segment

If your plugin provides state that should show in the prompt (like git status
or kubectl context), you need to:

1. Cache state in a zsh assoc array from your hook functions
2. The precmd hook in `shell/core/hooks.zsh` serializes known cache arrays to env
   vars before calling `lx prompt render`

If your plugin uses a custom cache key, you need to serialize it yourself in
your `_<pluginname>_plugin_precmd` function:

```zsh
_my_plugin_precmd() {
  __my_plugin_refresh_state
  # Serialize for lx prompt render
  export LYNX_CACHE_MY_STATE="{\"key\":\"${_my_plugin_state[key]:-}\"}"
}
```

Then implement a Rust segment in `lynx-prompt` that reads `cache["my_state"]`.
See `crates/lynx-prompt/src/segments/` for examples (`kubectl.rs` is the
simplest cache-reading segment).

Segments must implement `lynx_prompt::segment::Segment`:

```rust
pub struct MySegment;

impl Segment for MySegment {
    fn name(&self) -> &'static str { "my_segment" }
    fn cache_key(&self) -> Option<&'static str> { Some("my_state") }

    fn render(&self, config: &SegmentConfig, ctx: &RenderContext) -> Option<RenderedSegment> {
        let state = ctx.cache.get("my_state")?;
        let value = state.get("key")?.as_str()?;
        Some(RenderedSegment::new(value))
    }
}
```

---

## Testing Your Plugin

### Unit test zsh functions

Use bats (Bash Automated Testing System) for shell-level tests:

```bash
# tests/test_my_plugin.bats
setup() {
  export HOME="$(mktemp -d)"
  export LYNX_PLUGIN_DIR="$PWD"
  source shell/functions.zsh
}

teardown() { rm -rf "$HOME"; }

@test "my_func outputs expected result" {
  run my_func
  [ "$status" -eq 0 ]
  [[ "$output" == *"doing:"* ]]
}
```

Run with:
```bash
bats tests/test_my_plugin.bats
```

### Test in isolation

Install your plugin locally and verify it loads:

```bash
lx plugin add ./my-plugin
lx plugin list
lx doctor
```

### Check namespace compliance

```bash
lx doctor   # warns if any unexported function is defined
```

---

## Worked Example: weather plugin

This plugin fetches weather on directory change and shows it in the prompt.
It demonstrates: hooks, state caching, lazy loading, binary dep checking.

### plugin.toml

```toml
[plugin]
name        = "weather"
version     = "0.1.0"
description = "Shows current weather in the prompt"
authors     = ["Your Name <you@example.com>"]

[load]
lazy  = true             # don't slow down init — load on first use
hooks = ["chpwd"]        # refresh when changing directories

[deps]
binaries = ["curl", "jq"]   # checked before loading — skip cleanly if missing

[exports]
functions = ["weather_current", "weather_refresh"]
aliases   = ["wt"]

[contexts]
disabled_in = ["agent", "minimal"]
```

### shell/init.zsh

```zsh
# weather — init.zsh (keep ≤10 lines)
source "${LYNX_PLUGIN_DIR}/weather/shell/functions.zsh"
source "${LYNX_PLUGIN_DIR}/weather/shell/aliases.zsh"
```

### shell/functions.zsh

```zsh
# weather — functions.zsh
typeset -gA _weather_state
typeset -g  _weather_last_dir=""

# Hook target — registered via plugin.toml hooks[]
_weather_plugin_chpwd() {
  # Only refresh if directory changed (avoid redundant fetches)
  [[ "$PWD" != "$_weather_last_dir" ]] || return 0
  _weather_last_dir="$PWD"
  __weather_fetch_async
}

# Public function — fetch weather for current location
weather_refresh() {
  __weather_fetch_async
}

# Public function — print current weather
weather_current() {
  local temp="${_weather_state[temp]:-}"
  local desc="${_weather_state[desc]:-}"
  [[ -z "$temp" ]] && echo "weather: no data" && return 0
  echo "${temp}°C ${desc}"
}

# Internal — fetch in background, update state on completion
__weather_fetch_async() {
  local result
  result=$(curl -sf "https://wttr.in/?format=%t+%C" 2>/dev/null) || return 0
  local temp="${result%% *}"
  local desc="${result#* }"
  _weather_state=(temp "${temp//+/}" desc "$desc")
  # Serialize for lx prompt render (if you add a weather segment)
  export LYNX_CACHE_WEATHER_STATE="{\"temp\":\"${_weather_state[temp]}\",\"desc\":\"${_weather_state[desc]}\"}"
}
```

### shell/aliases.zsh

```zsh
# weather — aliases.zsh
alias wt='weather_current'
```

### Testing

```bash
bats tests/test_weather.bats
```

```bash
# tests/test_weather.bats
setup() {
  export HOME="$(mktemp -d)"
  export LYNX_PLUGIN_DIR="$BATS_TEST_DIRNAME/.."
  source shell/functions.zsh
}
teardown() { rm -rf "$HOME"; }

@test "weather_current shows no-data message when cache empty" {
  run weather_current
  [ "$status" -eq 0 ]
  [[ "$output" == *"no data"* ]]
}

@test "weather_current shows data after state is set" {
  _weather_state=(temp "18" desc "Sunny")
  run weather_current
  [[ "$output" == *"18"* ]]
  [[ "$output" == *"Sunny"* ]]
}
```

---

## Submitting to the Registry

The Lynx plugin registry is a static TOML index hosted on GitHub. See
[Registry Index Spec](registry-index-spec.md) for the format.

To submit your plugin:

1. Publish your plugin to a public git repo
2. Ensure `plugin.toml` passes `lx plugin add ./your-plugin` validation
3. Open a PR to [lynx-plugins](https://github.com/proxikal/lynx-plugins)
   adding an entry to `index.toml`
4. The registry maintainers will review for: manifest completeness, namespace
   safety, binary dep declarations, and context gating correctness

Quality bar for registry plugins:
- `disabled_in = ["agent", "minimal"]` set for aliases
- All exported symbols listed in `[exports]`
- `[deps].binaries` lists every required binary
- Plugin loads cleanly via `lx doctor` on a fresh install
- At least one bats test included
