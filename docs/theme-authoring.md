# Theme Authoring Guide

A Lynx theme is a TOML file that owns the **complete shell visual experience**:
prompt segments, prompt character, file listing colors (`LS_COLORS`, `EZA_COLORS`),
and more surfaces as the system grows. Switching a theme changes everything — not
just the prompt.

This guide covers the full theme schema, every available segment, and how to add
a new segment in Rust.

> **Note on planned segments:** This guide only documents segments that are
> fully implemented. Segments marked as *planned* exist in the roadmap
> (tracked in `pt issues P2 themes`) but are not yet available.

---

## Design Philosophy (vs. Starship / OMZ)

| Concern | Starship / OMZ | Lynx |
|---|---|---|
| **Prompt config** | TOML / zsh | TOML |
| **File listing colors** | Manual (`LS_COLORS` DIY) | Defined in theme `[ls_colors]` |
| **Color palette** | Per-segment hex values | `[colors]` palette, referenced by name (`$accent`) |
| **Segment visibility** | Per-segment flags, inconsistent | Universal `show_in` / `hide_in` on every segment |
| **Segment config** | Shared config blocks | Each segment owns its typed config |
| **Context awareness** | Limited / opt-in | First-class (interactive / agent / minimal) |

The key difference: **one theme switch = cohesive terminal**. Colors, prompt,
and file listings are all derived from the same palette.

---

## Table of Contents

1. [Design Philosophy](#design-philosophy-vs-starship--omz)
2. [Theme File Structure](#theme-file-structure)
3. [Palette System](#palette-system)
4. [Segment Layout](#segment-layout)
5. [Universal Visibility](#universal-visibility)
6. [Segment Format Strings](#segment-format-strings)
7. [Custom Template Segments](#custom-template-segments)
8. [Segment Reference](#segment-reference)
9. [File Listing Colors](#file-listing-colors)
10. [Color Formats](#color-formats)
11. [Worked Example: Powerline-Style Theme](#worked-example-powerline-style-theme)
12. [Testing Your Theme](#testing-your-theme)
13. [Adding a Custom Segment in Rust](#adding-a-custom-segment-in-rust)

---

## Theme File Structure

Themes live in `~/.config/lynx/themes/<name>.toml` or in the bundled
`themes/` directory of the Lynx repository.

```toml
[meta]
name        = "my-theme"          # required — must match the filename
description = "A clean theme"     # shown in lx theme list
author      = "Your Name"

# ── Palette ─────────────────────────────────────────────────────────────────
# Single source of truth for all colors. Segment configs reference these by
# name via $variable syntax. Hex, named colors, and xterm-256 indices all work.
[colors]
accent  = "#7aa2f7"
success = "#9ece6a"
warning = "#e0af68"
error   = "#f7768e"
muted   = "#565f89"
fg      = "#c0caf5"
bg      = "#1a1b26"

# ── Prompt layout ────────────────────────────────────────────────────────────
[segments.left]
order = ["dir", "git_branch", "git_status", "prompt_char"]  # left prompt

[segments.right]
order = ["cmd_duration", "context_badge"]                   # right prompt

# ── Per-segment config ────────────────────────────────────────────────────────
# All fields optional. Colors reference palette vars ($name) or literal values.
# Every segment also accepts: show_in = ["interactive"] / hide_in = ["agent"]
[segment.dir]
max_depth = 3
color     = { fg = "$accent", bold = true }

[segment.git_branch]
icon  = " "
color = { fg = "$warning" }

[segment.prompt_char]
symbol       = "❯"
error_symbol = "❯"
color        = { fg = "$success" }
error_color  = { fg = "$error" }

# ── File listing colors ───────────────────────────────────────────────────────
# Emitted as LS_COLORS and EZA_COLORS on theme switch / shell init.
# Uses semantic keys — palette vars supported. (H-054: in progress)
[ls_colors]
dir        = { fg = "$accent", bold = true }
symlink    = { fg = "#89ddff" }
executable = { fg = "$success", bold = true }
archive    = { fg = "$warning" }
image      = { fg = "#ff007c" }
audio      = { fg = "#ff007c" }
broken     = { fg = "$error" }
```

Switch to your theme with:
```bash
lx theme switch my-theme
lx theme list        # shows all available themes
```

---

## Palette System

The `[colors]` table is the single source of truth for all colors in a theme.
Segment color configs reference palette keys by name using `$variable` syntax:

```toml
[colors]
accent  = "#7aa2f7"
error   = "#f7768e"
success = "#9ece6a"

[segment.dir]
color = { fg = "$accent" }          # resolves to "#7aa2f7"

[segment.prompt_char]
color       = { fg = "$success" }
error_color = { fg = "$error" }
```

**Rules (D-015):**
- Segment configs MUST reference palette vars rather than hardcoding hex values.
- The `[colors]` table is the only place raw color values should appear.
- This makes the entire theme recolorable by changing `[colors]` only.

Standard semantic palette keys (used by built-in segments and `[ls_colors]`):

| Key | Purpose |
|---|---|
| `accent` | Primary highlight (dirs, branches) |
| `success` | Success state (clean git, prompt char) |
| `warning` | Warning state (git dirty, slow cmd) |
| `error` | Error state (prompt char on failure, broken symlinks) |
| `muted` | Subdued text (timestamps, durations) |
| `fg` | Default foreground |
| `bg` | Default background (used for Powerline fill) |

All keys are optional — segments fall back to terminal defaults if a palette
key is absent.

---

## Segment Layout

The `[segments.left]` and `[segments.right]` tables each have an `order` array.
Segments are evaluated **concurrently** and assembled in the declared order.

```toml
[segments.left]
order = ["dir", "git_branch", "git_status"]
```

- Segments not listed are not evaluated (no performance cost)
- A segment that returns nothing (e.g. `git_branch` outside a git repo) is
  silently omitted — no gap in the prompt
- The same segment cannot appear in both `left` and `right` (use it once)

---

## Universal Visibility

Every segment accepts `show_in` and `hide_in` fields in its `[segment.*]` config.
These are evaluated by the renderer before the segment is called — a hidden
segment costs zero evaluation time (D-017).

```toml
[segment.username]
show_in = ["interactive"]       # only show in interactive context

[segment.profile_badge]
hide_in = ["agent", "minimal"]  # hide in agent and minimal

[segment.context_badge]
show_in = ["agent", "minimal"]  # always shown (overrides hide_in)
```

Valid context values: `interactive`, `agent`, `minimal`.

### Condition-based visibility (`show_when` / `hide_when`)

For finer control beyond shell context, use `show_when` or `hide_when`. These
accept a single inline table with one condition — evaluated against the current
environment and prompt state, with no I/O.

```toml
[segment.username]
show_when = { env_set = "SSH_CONNECTION" }   # only over SSH

[segment.git_branch]
show_when = { in_git_repo = true }           # only inside git repos

[segment.venv]
show_when = { env_matches = { VIRTUAL_ENV = "*myproject*" } }

[segment.dir]
show_when = { cwd_matches = "~/work/**" }    # ~ expanded from HOME env var

[segment.exit_code]
show_when = { exit_code_nonzero = true }     # only on non-zero exit

[segment.ci_badge]
hide_when = { env_set = "CI" }              # hidden in CI environments
```

**Condition types:**

| Condition | Type | Matches when… |
|---|---|---|
| `env_set` | `string` | named env var is set and non-empty |
| `env_matches` | `{ VAR = "glob" }` | all listed env vars match their glob patterns |
| `in_git_repo` | `true` / `false` | git cache present (`true`) or absent (`false`) |
| `cwd_matches` | `string` (glob) | current directory matches pattern (`~` expanded) |
| `exit_code_nonzero` | `true` / `false` | last exit code non-zero (`true`) or zero (`false`) |

**Priority:** `show_in` / `hide_in` (context gate) is evaluated first. If the
segment passes the context gate, `show_when` is checked next (takes priority
over `hide_when`). Conditions are pure — no I/O, no subprocess calls.

---

## Segment Format Strings

Every segment that produces text supports an optional `format` field. Format
strings use `$variable` substitution — the same syntax as Starship modules.

```toml
[segment.git_branch]
format = "[$branch]($style) "   # wrap branch in brackets, icon omitted
```

**Rules:**
- `$variable` expands to the segment's value for that variable
- Unknown variable names expand to an empty string (no error, no panic)
- `$$` produces a literal `$`
- If `format` is absent, the segment uses its built-in default layout

**Available variables per segment:**

| Segment | Variables |
|---|---|
| `git_branch` | `$icon`, `$branch` |
| `git_status` | `$staged`, `$modified`, `$untracked` |
| `dir` | `$path` |
| `cmd_duration` | `$duration` |
| `git_ahead_behind` | `$ahead`, `$behind` |

`$staged`, `$modified`, `$untracked`, `$ahead`, and `$behind` expand to the
icon/count when the condition is true, or to an empty string when false.

**Examples:**

```toml
# git_branch: icon after branch, in angle brackets
[segment.git_branch]
icon   = "⎇ "
format = "[$branch] $icon"

# git_status: wrap all icons in square brackets
[segment.git_status]
format = "[$staged$modified$untracked]"

# cmd_duration: add a prefix label
[segment.cmd_duration]
format = "took $duration"

# git_ahead_behind: custom arrangement with a pipe separator
[segment.git_ahead_behind]
format = "$ahead|$behind"
```

---

## Custom Template Segments

Custom template segments let you compose one-off prompt segments entirely in
TOML — no Rust required. Any name starting with `custom_` in a segment order
array is treated as a custom segment.

```toml
[segments.left]
order = ["custom_greeting", "dir", "git_branch"]

[segment.custom_greeting]
template = "hello ${env.USER}"
color    = { fg = "blue" }
show_in  = ["interactive"]
```

### Template syntax

| Form | Expands to |
|---|---|
| `$cwd` | Current working directory |
| `$context` | Shell context: `interactive`, `agent`, or `minimal` |
| `$last_cmd_ms` | Last command duration in ms, or empty string |
| `${env.VAR}` | Environment variable from the context snapshot |
| `${cache.PLUGIN.FIELD}` | Field from a plugin's JSON state cache |
| `$$` | A literal `$` |

Unknown variables and missing cache fields expand to an empty string — they
never produce an error. If the entire rendered output is empty, the segment is
hidden (same as returning `None` from a Rust segment).

### Available cache fields

Cache fields use the same JSON structure each plugin writes to its
`LYNX_CACHE_<PLUGIN>_STATE` variable. Common examples:

| Template var | What it shows |
|---|---|
| `${cache.git.branch}` | Current git branch |
| `${cache.git.dirty}` | `1` when repo has uncommitted changes |
| `${cache.git.ahead}` | Commits ahead of upstream |
| `${cache.git.behind}` | Commits behind upstream |
| `${cache.kubectl.context}` | Active kubectl context |
| `${cache.kubectl.namespace}` | Active kubectl namespace |
| `${cache.node.version}` | Node.js version from `.node-version`/`.nvmrc` |

### Universal visibility

Custom segments support `show_in` and `hide_in` exactly like any built-in
segment — the evaluator applies visibility rules before rendering:

```toml
[segment.custom_ssh_user]
template = "${env.USER}@${env.HOSTNAME}"
show_in  = ["interactive"]    # only show in interactive sessions
```

### When to use custom segments vs Rust segments

| Situation | Approach |
|---|---|
| Compose existing RenderContext data in a new way | Custom template segment |
| Display a static label with context-based visibility | Custom template segment |
| Need to call an external tool or read a file | Rust segment (no I/O in render) |
| Need complex logic, formatting, or color per-field | Rust segment |

---

## Segment Reference

### `dir` — Current Directory

Shows the current working directory, optionally shortened.

| Field | Type | Default | Description |
|---|---|---|---|
| `max_depth` | integer | `3` | Max path components to show. `0` = show full path |
| `truncate_to_repo` | bool | `true` | When in a git repo, show path relative to repo root |
| `format` | string | `"$path"` | Format template — see [Segment Format Strings](#segment-format-strings) |
| `color` | color | none | Text color |

```toml
[segment.dir]
max_depth = 3
truncate_to_repo = true
color = { fg = "blue", bold = true }
```

**Example output:** `~/code/lynx/crates/core`

---

### `git_branch` — Current Branch Name

Shows the current git branch. Hidden outside a git repo.

Requires: `git_state` cache populated by the git plugin (`add lx plugin add git`).

| Field | Type | Default | Description |
|---|---|---|---|
| `icon` | string | `" "` | Prefix icon before branch name |
| `format` | string | `"$icon$branch"` | Format template — vars: `$icon`, `$branch` |
| `color` | color | none | Text color |

```toml
[segment.git_branch]
icon = " "
color = { fg = "yellow" }
```

**Example output:** ` main`

---

### `git_status` — Dirty/Staged/Untracked Indicators

Shows icons for staged, modified, and untracked files. Hidden when working
tree is clean or outside a git repo.

Requires: `git_state` cache (git plugin).

| Field | Type | Default | Description |
|---|---|---|---|
| `staged.icon` | string | `"+"` | Icon for staged files |
| `staged.color` | color string | none | Color for staged icon |
| `modified.icon` | string | `"!"` | Icon for modified files |
| `modified.color` | color string | none | Color for modified icon |
| `untracked.icon` | string | `"?"` | Icon for untracked files |
| `untracked.color` | color string | none | Color for untracked icon |
| `format` | string | `"$staged$modified$untracked"` | Format template — vars: `$staged`, `$modified`, `$untracked` |

```toml
[segment.git_status]
staged    = { icon = "✚", color = "green" }
modified  = { icon = "✎", color = "red" }
untracked = { icon = "…", color = "grey" }
```

**Example output:** `✚✎` (staged and modified files present)

---

### `cmd_duration` — Last Command Duration

Shows how long the previous command took. Hidden when below the threshold.

| Field | Type | Default | Description |
|---|---|---|---|
| `min_ms` | integer | `500` | Minimum duration (ms) before showing |
| `format` | string | `"$duration"` | Format template — vars: `$duration` |
| `color` | color | none | Text color |

```toml
[segment.cmd_duration]
min_ms = 500
color  = { fg = "grey" }
```

**Example output:** `2.3s` / `1m45s` / `450ms` (when over threshold)

---

### `git_ahead_behind` — Ahead/Behind Remote

Shows how many commits the current branch is ahead or behind its remote tracking
branch. Hidden when both counts are zero or when no remote is configured.

Requires: `git_state` cache (git plugin).

| Field | Type | Default | Description |
|---|---|---|---|
| `ahead_symbol` | string | `"↑"` | Symbol for ahead count |
| `behind_symbol` | string | `"↓"` | Symbol for behind count |
| `format` | string | `"$ahead $behind"` | Format template — vars: `$ahead`, `$behind` (each empty when zero) |
| `color` | color | none | Text color |

```toml
[segment.git_ahead_behind]
ahead_symbol  = "⇡"
behind_symbol = "⇣"
```

**Example output:** `↑2` / `↓3` / `↑1 ↓2`

---

### `kubectl_context` — Kubernetes Context

Shows the active kubectl context and namespace. Hidden when kubectl is not
installed, no context is active, or the context is `"default"`.

Requires: `kubectl_state` cache populated by the kubectl plugin.

| Field | Type | Default | Description |
|---|---|---|---|
| `prod_pattern` | regex string | none | Contexts matching this pattern are marked `[PROD]` |
| `color` | color | none | Text color |

```toml
[segment.kubectl_context]
prod_pattern = "prod.*"
color = { fg = "cyan" }
```

**Example output:** `⎈ staging:api-ns` / `[PROD] ⎈ prod-us-east:default`

---

### `profile_badge` — Active Profile Name

Shows the active Lynx profile name. Hidden in agent and minimal contexts,
and when no profile is active.

| Field | Type | Default | Description |
|---|---|---|---|
| `icon` | string | `"⬡ "` | Prefix icon |
| `color` | color | none | Text color |

```toml
[segment.profile_badge]
icon  = "⬡ "
color = { fg = "magenta" }
```

**Example output:** `⬡ work`

---

### `task_status` — Running Background Tasks

Shows a count of running Lynx background tasks. Hidden when no tasks are running.

| Field | Type | Default | Description |
|---|---|---|---|
| `color` | color | none | Text color |

```toml
[segment.task_status]
color = { fg = "yellow" }
```

**Example output:** `↻ 2` (two tasks running)

---

### `context_badge` — Shell Context Indicator

Shows a badge when running in agent or minimal context. Useful for knowing at
a glance that aliases are not loaded.

| Field | Type | Default | Description |
|---|---|---|---|
| `show_in` | string array | `["agent", "minimal"]` | Contexts where badge appears |
| `label` | map | `{agent="AI", minimal="MIN"}` | Badge text per context |
| `color` | color | none | Text color |

```toml
[segment.context_badge]
show_in = ["agent"]
label   = { agent = "AI", minimal = "MIN" }
color   = { fg = "magenta", bold = true }
```

**Example output:** `AI` (when in agent context)

---

### `git_sha` — Commit SHA

Shows the short commit SHA from the current git repository.

| Field | Type | Default | Description |
|---|---|---|---|
| `length` | integer | `7` | Number of hex characters to display (max 40) |
| `prefix` | string | `""` | Prefix before the SHA |
| `color` | color | none | Text color |

```toml
[segment.git_sha]
length = 7
color = { fg = "grey" }
```

**Example output:** `abc1234`

---

### `git_time_since_commit` — Commit Age

Shows time elapsed since the last commit with color aging (green → yellow → red).

| Field | Type | Default | Description |
|---|---|---|---|
| `fresh_secs` | integer | `600` | Seconds before time is "fresh" (green) |
| `warn_secs` | integer | `1800` | Seconds before time is "warn" (yellow) |
| `fresh_color` | string | `"green"` | Color for fresh commits |
| `warn_color` | string | `"yellow"` | Color for warning-age commits |
| `old_color` | string | `"red"` | Color for old commits |

**Example output:** `5m` (green), `20m` (yellow), `2h0m` (red), `3d` (red)

---

### `aws_profile` — AWS Profile

Shows the current AWS profile from `$AWS_PROFILE` with production detection.
Zero I/O — reads only the environment variable.

| Field | Type | Default | Description |
|---|---|---|---|
| `prod_patterns` | string array | `["*prod*", "*production*"]` | Patterns that identify production profiles |
| `color` | string | `"green"` | Color for non-production profiles |
| `prod_color` | string | `"red"` | Color for production profiles (also bold) |
| `icon` | string | `""` | Icon prepended to the profile name |

```toml
[segment.aws_profile]
icon = "☁"
prod_patterns = ["*prod*", "*production*"]
```

**Example output:** `☁ staging` (green), `☁ my-prod-account` (red, bold)

---

### `hist_number` — Shell History Number

Shows the current shell history number from `$HISTCMD`.

| Field | Type | Default | Description |
|---|---|---|---|
| `prefix` | string | `""` | Prefix before the history number |
| `color` | color | none | Text color |

```toml
[segment.hist_number]
prefix = "!"
color = { fg = "grey" }
```

**Example output:** `!42`

---

### `prompt_char` — Prompt Character

Shows a configurable prompt character with context-aware variants.

| Field | Type | Default | Description |
|---|---|---|---|
| `symbol` | string | `"❯"` | Default prompt symbol |
| `error_symbol` | string | (falls back to `symbol`) | Symbol when last command failed |
| `root_symbol` | string | (falls back to `symbol`) | Symbol when running as root |
| `in_git_repo_symbol` | string | (falls back to `symbol`) | Symbol when inside a git repo |

Priority order: error > root > git_repo > default.

```toml
[segment.prompt_char]
symbol = "❯"
error_symbol = "✗"
root_symbol = "#"
in_git_repo_symbol = "±"
```

---

## File Listing Colors

> **Status:** Planned — tracked in H-054. The schema below is the target design.
> Once implemented, `lx theme switch` will emit `LS_COLORS` and `EZA_COLORS`
> automatically.

The `[ls_colors]` table lets a theme own `ls`, `eza`, and `lsd` output — the same
palette variables available to segments apply here.

```toml
[ls_colors]
dir        = { fg = "$accent", bold = true }
symlink    = { fg = "#89ddff" }
executable = { fg = "$success", bold = true }
archive    = { fg = "$warning" }
image      = { fg = "#ff007c" }
audio      = { fg = "#ff007c" }
broken     = { fg = "$error" }        # broken symlink
other_writable = { fg = "$warning" }  # world-writable dir
```

Semantic keys and their `LS_COLORS` mappings:

| Key | LS_COLORS code | Notes |
|---|---|---|
| `dir` | `di` | Directories |
| `symlink` | `ln` | Symbolic links |
| `executable` | `ex` | Executable files |
| `archive` | — | `.tar`, `.gz`, `.zip`, etc. (extension list) |
| `image` | — | `.png`, `.jpg`, `.gif`, etc. |
| `audio` | — | `.mp3`, `.flac`, `.wav`, etc. |
| `broken` | `or` | Broken symlinks |
| `other_writable` | `ow` | Dirs writable by others |

If `[ls_colors]` is absent, Lynx emits no `LS_COLORS` (OS default applies).

---

## Color Formats

Colors can be specified three ways:

### Named colors

Standard terminal color names. Always available, regardless of terminal capability.

```
"black"   "red"     "green"  "yellow"
"blue"    "magenta" "cyan"   "white"
"grey"    "default"
```

### Hex colors (truecolor)

```toml
color = { fg = "#7aa2f7" }
```

Requires a truecolor terminal. Lynx auto-detects terminal capability and
downgrades to the closest 256-color match if truecolor is not available.

### ANSI 256

Not supported as a direct input format — use named or hex. Lynx handles
downgrading automatically.

### Color object fields

```toml
color = { fg = "#7aa2f7", bold = true }
color = { fg = "red", bold = false }
```

| Field | Type | Description |
|---|---|---|
| `fg` | string | Foreground color (named or hex) |
| `bold` | bool | Bold text (default `false`) |
| `bg` | string | Background color (named or hex) — not all segments use this |

---

## Separators

The `[separators]` table controls the glyphs and colors between segments.

| Field | Type | Default | Description |
|---|---|---|---|
| `mode` | string | `"static"` | `"static"` (one style for all gaps) or `"adaptive"` (per-gap colors from adjacent bg) |
| `left.char` | string | `" "` | Character between left segments |
| `left.color` | string | none | Foreground color of the left separator (static mode) |
| `right.char` | string | `" "` | Character between right segments |
| `left_edge.char` | string | `""` | Leading edge before first left segment |
| `right_edge.char` | string | `""` | Trailing edge after last left segment |

### Adaptive mode (powerline)

When `mode = "adaptive"`, the renderer computes separator colors automatically:
- The separator between two segments uses **fg = previous segment's bg** and **bg = next segment's bg**
- After the last bg-colored segment, a "tail arrow" is emitted with **fg = last segment's bg** and no bg
- Segments without a bg color fall back to the static separator color

```toml
[separators]
mode = "adaptive"

[separators.left]
char = "\ue0b0"  # Powerline right chevron
```

This requires segments to have `bg` colors set in their `color` config.

---

## Theme Convert (OMZ Import)

Convert Oh My Zsh themes to Lynx TOML format:

```bash
# From a GitHub URL
lx theme convert https://github.com/ohmyzsh/ohmyzsh/blob/master/themes/candy.zsh-theme candy

# From a local file
lx theme convert ./mytheme.zsh-theme mytheme

# Overwrite existing
lx theme convert ./mytheme.zsh-theme mytheme --force
```

The converter maps OMZ `%` tokens and `$(function)` calls to Lynx segments.
Agnoster-style themes produce partial output with notes for manual tuning.

---

## Worked Example: Powerline-Style Theme

This theme uses Powerline glyphs and a dark color palette to produce a
classic prompt style. Save as `~/.config/lynx/themes/powerline.toml`.

```toml
[meta]
name        = "powerline"
description = "Powerline-style prompt with glyphs"
author      = "you"

[segments.left]
order = ["dir", "git_branch", "git_status"]

[segments.right]
order = ["kubectl_context", "profile_badge", "cmd_duration", "task_status", "context_badge"]

[segment.dir]
max_depth        = 3
truncate_to_repo = true
color            = { fg = "#c0caf5", bold = true }

[segment.git_branch]
icon  = " "
color = { fg = "#e0af68" }

[segment.git_status]
staged    = { icon = "+", color = "green" }
modified  = { icon = "~", color = "red" }
untracked = { icon = "?", color = "#565f89" }

[segment.kubectl_context]
prod_pattern = "prod.*"
color        = { fg = "#7dcfff" }

[segment.profile_badge]
icon  = " "
color = { fg = "#bb9af7" }

[segment.cmd_duration]
min_ms = 1000
color  = { fg = "#565f89" }

[segment.task_status]
color = { fg = "#e0af68" }

[segment.context_badge]
show_in = ["agent", "minimal"]
label   = { agent = "[AI]", minimal = "[min]" }
color   = { fg = "#f7768e", bold = true }

[colors]
accent  = "#7aa2f7"
success = "#9ece6a"
warning = "#e0af68"
error   = "#f7768e"
muted   = "#565f89"
```

Activate it:
```bash
lx theme switch powerline
```

---

## CLI Theme Customization

Lynx provides three layers of CLI customization — all write to the same TOML
file through the same snapshot/validate/rollback pipeline:

### Layer 1 — Convenience shorthands (humans)

```bash
lx theme caret "❯"                    # change prompt character
lx theme caret-color light-blue       # change caret color (named or hex)
lx theme palette accent "#7aa2f7"     # change a palette key
lx theme palette error dark-red       # named colors work everywhere

lx theme segment remove cmd_duration  # hide a segment entirely
lx theme segment move git_branch right  # move to right side
lx theme segment add venv left --after dir  # insert after dir on left
```

### Layer 2 — Surgical patch (power users and AI agents)

`lx theme patch <dot.path> <value>` mutates any scalar field in the active
theme TOML by dot-separated path:

```bash
# Colors
lx theme patch colors.accent light-blue
lx theme patch segment.dir.color.fg "#7aa2f7"
lx theme patch segment.git_branch.color.bold true

# Structure
lx theme patch segment.git_branch.icon "["
lx theme patch segment.git_status.staged.icon "+"

# Visibility
lx theme patch segment.username.show_in '["interactive"]'
lx theme patch segment.context_badge.hide_in '["minimal"]'
```

All patch operations: snapshot → apply → validate → rollback on failure →
emit `theme:changed`. Theme files live in `~/.config/lynx/themes/` (set up
by `lx setup`).

### Layer 3 — Full editor

```bash
lx theme edit   # opens active theme in $EDITOR with rollback on bad save
```

### lx theme studio (WYSIWYG — planned, H-062)

```bash
lx theme studio   # starts a local web server, opens browser
```

A local web UI with live prompt preview, drag-and-drop segment ordering,
color pickers backed by the named color registry, and one-click apply.
No npm, no build step — ships embedded in the binary.

---

## Testing Your Theme

Validate the TOML schema:
```bash
lx theme list    # will error if your theme TOML is malformed
```

Preview without switching:
```bash
lx theme switch powerline
# Open a new terminal to see it
```

Check for unknown segment names:
```bash
lx doctor    # warns about segments in your theme not known to Lynx
```

---

## Adding a Custom Segment in Rust

> **Before writing Rust:** If you only need to compose existing RenderContext
> data (env vars, cache fields, cwd, context), use a
> [custom template segment](#custom-template-segments) — no code required.

Rust segments are for cases that need I/O-free logic beyond what templates
support: complex conditional formatting, per-field colors, or new data sources
that require a matching plugin. This section is for contributors adding a
segment to the core.

### 1. Create the segment file

In `crates/lynx-prompt/src/segments/my_segment.rs`:

```rust
use crate::segment::{RenderContext, RenderedSegment, Segment};

pub struct MySegment;

impl Segment for MySegment {
    fn name(&self) -> &'static str {
        "my_segment"    // must match the key used in theme TOML [segment.*]
    }

    fn cache_key(&self) -> Option<&'static str> {
        None    // or Some("my_state") if you read from the cache
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        // Return None to hide the segment entirely.
        // Return Some(RenderedSegment::new("text")) to show it.
        Some(RenderedSegment::new("hello"))
    }
}
```

**Rules:**
- `render` must not perform I/O — read from `ctx.cache` only
- `render` must not block — it runs inside a `tokio::join`
- Return `None` to hide (not an empty string)

### 2. Register the segment

In `crates/lynx-prompt/src/segments/mod.rs`, add:

```rust
mod my_segment;
pub use my_segment::MySegment;
```

### 3. Add to KNOWN_SEGMENTS

In `crates/lynx-theme/src/schema.rs`:

```rust
pub const KNOWN_SEGMENTS: &[&str] = &[
    // ... existing segments ...
    "my_segment",
];
```

### 4. Register in the CLI

In `crates/lynx-cli/src/commands/prompt.rs`, add your segment to the registry:

```rust
let segments: Vec<Box<dyn lynx_prompt::segment::Segment>> = vec![
    // ... existing segments ...
    Box::new(MySegment),
];
```

### 5. Add a unit test

Every segment needs tests for: hidden when no data, shows when data present,
handles edge cases. See `segments/kubectl.rs` for a complete example.

### 6. Document it

Add your segment to the [Segment Reference](#segment-reference) table in this file.
