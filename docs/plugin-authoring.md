# Plugin Authoring Guide

Lynx plugins are self-contained directories with a `plugin.toml` manifest,
a thin zsh shell layer, and any Rust prompt segments they need. This guide
walks you through building a real plugin from scratch.

> **Stable API reference:** See [plugin-api.md](plugin-api.md) for the
> complete list of stable env vars, lx commands, and shell variable contracts
> that plugins may depend on.

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
hooks = []       # per-plugin hook registration is not used — lx refresh-state handles precmd

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

[state]
# Optional. Community plugins declare a gather command here.
# lx refresh-state calls this command once per precmd (single process spawn total).
# $PLUGIN_DIR is set to your plugin's directory before the command runs.
# Output must be valid zsh. Convention: export LYNX_CACHE_<NAME>_STATE as a JSON
# string and populate the _lynx_<name>_state assoc array.
# The command can be any executable: shell script, Go binary, Python, Rust binary, etc.
# First-party plugins (git, kubectl) use native Rust gatherers — no gather needed.
gather = ""    # e.g. "$PLUGIN_DIR/bin/my-plugin-state" or "zsh $PLUGIN_DIR/gather.zsh"

[shell]
# Optional. All fields default to empty — omit the entire section if unused.
#
# fpath: directories relative to the plugin root prepended to $fpath BEFORE
# init.zsh is sourced. Use this to register zsh completions. Convention:
# put completion files in completions/ using the _command naming format.
fpath = ["completions"]
#
# widgets: ZLE widget names to register with `zle -N`. The widget function
# must be defined in shell/functions.zsh and listed in exports.functions.
# Registered after init.zsh is sourced.
widgets = ["my_plugin_widget"]
#
# keybindings: key → widget pairs registered with `bindkey` after all `zle -N`
# calls. key is a zsh key sequence (e.g. "^R", "\\eOA", "^[[A").
# widget must be declared in shell.widgets.
[[shell.keybindings]]
key    = "^F"
widget = "my_plugin_widget"
```

### Field details

**`[load].lazy`** — When `true`, Lynx registers a stub function for each
exported function. On first call, the stub sources `shell/init.zsh`, removes
itself, and replays the original call. Use lazy for plugins with heavy deps
(e.g., `kubectl`) where you don't want init-time overhead.

**`[load].hooks`** — Reserved for custom hook needs that fall outside normal state
refresh (e.g., `preexec` to capture command strings). For state that the prompt
needs, use `[state].gather` instead. State refresh is handled automatically by
`lx refresh-state`, which Lynx registers as the single shared precmd hook.
You do **not** need to declare `chpwd` or `precmd` here for state caching.

**`[state].gather`** — Command Lynx runs inside `lx refresh-state` on each precmd.
The command receives `$PLUGIN_DIR` pointing to your plugin's directory. Output
must be valid zsh. See [Hooks: Responding to Shell Events](#hooks-responding-to-shell-events)
for the full contract.

**`[deps].binaries`** — Lynx checks these with `command -v` before loading. If
any are missing, the plugin is skipped with a diagnostic (not an error). This
is better than loading and failing mysteriously.

**`[exports].functions`** — Every public function your plugin exposes. Internal
helpers must use the `__<pluginname>_` prefix convention (double underscore,
plugin name prefix) — they are not exported and will not be checked.

**`[exports].aliases`** — Every alias. These are only sourced when the context
is not in `[contexts].disabled_in`.

**`[shell].fpath`** — Relative paths within the plugin directory that Lynx
prepends to `$fpath` before sourcing `init.zsh`. The convention is a
`completions/` directory containing `_command`-named completion files. Do **not**
call `compinit` inside a plugin — Lynx calls it once during `lx init`.

**`[shell].widgets`** — ZLE widget names that Lynx registers with `zle -N`
after sourcing `init.zsh`. The widget function must be defined in
`shell/functions.zsh` and listed in `exports.functions`. Registering here
(not in init.zsh) keeps the shell layer logic-free (D-001).

**`[shell].keybindings`** — An array of `{key, widget}` tables. Lynx emits
`bindkey '<key>' <widget>` for each entry, in order, after all `zle -N`
registrations. The `key` field is any zsh key sequence string.

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

State refresh in Lynx is centralized. Plugins do not self-register `chpwd` or
`precmd` hooks. Instead, Lynx registers a single shared precmd hook that calls
`lx refresh-state` — one process spawn per prompt draw, regardless of how many
plugins are loaded.

### How state refresh works

```
precmd fires
  └─ lx refresh-state
       ├─ runs each plugin's state.gather command (community plugins)
       └─ runs native Rust gatherers (first-party plugins: git, kubectl, …)
```

The output of each gather command is `eval`'d by the shell. After all gather
commands complete, `lx prompt render` reads the populated env vars to build the
prompt.

### Community plugins: declare state.gather

Instead of implementing `_<name>_plugin_precmd()`, community plugins declare a
gather command in `plugin.toml`:

```toml
[state]
gather = "$PLUGIN_DIR/bin/my-plugin-state"
```

`$PLUGIN_DIR` is set to your plugin's directory before the command runs. The
command can be written in any language — shell script, Go binary, Python script,
compiled Rust binary, etc.

### The state.gather contract

Your gather command must write valid zsh to stdout. The expected output pattern:

```zsh
# Export a JSON string for lx prompt render (cache key = lowercase plugin name)
export LYNX_CACHE_MY_PLUGIN_STATE='{"key":"value","other":"data"}'
# Populate a zsh assoc array for shell-side consumers
typeset -gA _lynx_my_plugin_state
_lynx_my_plugin_state=(key "value" other "data")
```

Rules:
- Output must be valid zsh — it is passed to `eval`.
- The `LYNX_CACHE_<NAME>_STATE` env var value must be valid JSON.
- The gather command must be fast — it runs on every prompt. Offload slow I/O
  to a background process and write a cached result to a temp file.
- If the gather command exits non-zero, its output is discarded silently.

### First-party plugins

First-party plugins (git, kubectl) use native Rust gatherers compiled into `lx`.
They do not need a `[state].gather` entry — their state is always collected
by `lx refresh-state` as part of the built-in gather pass.

### Manual refresh

Users can force an immediate state refresh at any time:

```zsh
lx refresh-state
```

Your plugin can also expose a refresh helper that calls the gather command
directly for immediate feedback without waiting for the next precmd:

```zsh
my_plugin_refresh() {
  eval "$(PLUGIN_DIR="${LYNX_PLUGIN_DIR}/my-plugin" "$PLUGIN_DIR/bin/my-plugin-state")"
}
```

### Custom hooks (non-state use cases)

If your plugin needs to respond to `preexec` (e.g., to capture the command
string before it runs), declare it in `[load].hooks`:

```toml
[load]
hooks = ["preexec"]
```

Then define the function in `functions.zsh`:

```zsh
_my_plugin_preexec() {
  local cmd="$1"
  # respond to the command — e.g. record timing, log commands
}
```

`chpwd` and `precmd` should not appear in `[load].hooks` for state-caching
purposes — use `[state].gather` instead.

### Available hooks (for [load].hooks)

| Hook | Triggers when |
|---|---|
| `preexec` | Before each command runs (receives the command string as `$1`) |

`chpwd` and `precmd` are handled by `lx refresh-state` and should not be
registered per-plugin.

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

1. Declare a `[state].gather` command in `plugin.toml` — your gather command
   outputs the `LYNX_CACHE_<NAME>_STATE` export (valid JSON) to stdout.
2. `lx refresh-state` calls your gather command each precmd and `eval`'s the output.
   No manual serialization in `shell/core/hooks.zsh` is needed.

Example gather script (`bin/my-plugin-state`):

```zsh
#!/usr/bin/env zsh
# Gather script — output must be valid zsh
local value="computed_value"
print "export LYNX_CACHE_MY_STATE='{\"key\":\"${value}\"}'"
print "typeset -gA _lynx_my_plugin_state"
print "_lynx_my_plugin_state=(key '${value}')"
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
lazy  = true    # don't slow down init — load on first use
hooks = []      # per-plugin hook registration is not used — lx refresh-state handles precmd

[deps]
binaries = ["curl", "jq"]   # checked before loading — skip cleanly if missing

[exports]
functions = ["weather_current", "weather_refresh"]
aliases   = ["wt"]

[contexts]
disabled_in = ["agent", "minimal"]

[state]
# lx refresh-state calls this script each precmd.
# $PLUGIN_DIR is set to this plugin's directory before execution.
gather = "$PLUGIN_DIR/bin/weather-state"
```

### shell/init.zsh

```zsh
# weather — init.zsh (keep ≤10 lines)
source "${LYNX_PLUGIN_DIR}/weather/shell/functions.zsh"
source "${LYNX_PLUGIN_DIR}/weather/shell/aliases.zsh"
```

### bin/weather-state

This is the gather script called by `lx refresh-state` each precmd. It must be
executable (`chmod +x bin/weather-state`).

```zsh
#!/usr/bin/env zsh
# weather-state — gather script for the weather plugin
# Called by lx refresh-state; $PLUGIN_DIR is set by the caller.
# Output is eval'd by the shell — must be valid zsh.

# Use a cache file to avoid a network call on every prompt.
# The plugin's weather_refresh() function updates this cache file on demand.
local cache_file="${XDG_CACHE_HOME:-$HOME/.cache}/lynx/weather.json"

if [[ -f "$cache_file" ]]; then
  local cached
  cached=$(<"$cache_file")
  print "export LYNX_CACHE_WEATHER_STATE='${cached}'"
  print "typeset -gA _lynx_weather_state"
  # Populate assoc array from the two keys we store
  local temp desc
  temp=$(print -- "$cached" | command jq -r '.temp // empty' 2>/dev/null)
  desc=$(print -- "$cached" | command jq -r '.desc // empty' 2>/dev/null)
  print "_lynx_weather_state=(temp '${temp}' desc '${desc}')"
fi
```

### shell/functions.zsh

```zsh
# weather — functions.zsh
# No hook functions needed — state refresh is handled by bin/weather-state
# via lx refresh-state (called automatically each precmd).

# Public function — print current weather from state populated by gather script
weather_current() {
  local temp="${_lynx_weather_state[temp]:-}"
  local desc="${_lynx_weather_state[desc]:-}"
  [[ -z "$temp" ]] && echo "weather: no data" && return 0
  echo "${temp}°C ${desc}"
}

# Public function — fetch fresh weather data and update the cache file.
# Also triggers an immediate state refresh so the prompt updates right away
# without waiting for the next precmd cycle.
weather_refresh() {
  local cache_dir="${XDG_CACHE_HOME:-$HOME/.cache}/lynx"
  mkdir -p "$cache_dir"
  local result
  result=$(curl -sf "https://wttr.in/?format=%t+%C" 2>/dev/null) || return 0
  local temp="${result%% *}"
  local desc="${result#* }"
  local json="{\"temp\":\"${temp//+/}\",\"desc\":\"${desc}\"}"
  print -- "$json" > "${cache_dir}/weather.json"
  # Immediate refresh — re-run gather without waiting for next precmd
  eval "$(PLUGIN_DIR="${LYNX_PLUGIN_DIR}/weather" "${LYNX_PLUGIN_DIR}/weather/bin/weather-state")"
}
```

Note: `weather_refresh()` manually re-invokes the gather script for immediate
feedback. This is the recommended pattern when a user-triggered update should
reflect in the current prompt without waiting for the next precmd.

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
3. Open a PR to [lynx-plugins](https://github.com/lynx-sh/registry)
   adding an entry to `index.toml`
4. The registry maintainers will review for: manifest completeness, namespace
   safety, binary dep declarations, and context gating correctness

Quality bar for registry plugins:
- `disabled_in = ["agent", "minimal"]` set for aliases
- All exported symbols listed in `[exports]`
- `[deps].binaries` lists every required binary
- Plugin loads cleanly via `lx doctor` on a fresh install
- At least one bats test included
