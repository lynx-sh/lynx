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
6. [Segment Reference](#segment-reference)
7. [File Listing Colors](#file-listing-colors)
8. [Color Formats](#color-formats)
9. [Worked Example: Powerline-Style Theme](#worked-example-powerline-style-theme)
10. [Testing Your Theme](#testing-your-theme)
11. [Adding a Custom Segment in Rust](#adding-a-custom-segment-in-rust)

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

> **Future:** `show_when` / `hide_when` for condition-based visibility
> (SSH session, root user, etc.) — tracked in H-053.

---

## Segment Reference

### `dir` — Current Directory

Shows the current working directory, optionally shortened.

| Field | Type | Default | Description |
|---|---|---|---|
| `max_depth` | integer | `3` | Max path components to show. `0` = show full path |
| `truncate_to_repo` | bool | `true` | When in a git repo, show path relative to repo root |
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
| `color` | color | none | Text color |

```toml
[segment.cmd_duration]
min_ms = 500
color  = { fg = "grey" }
```

**Example output:** `2.3s` / `1m45s` / `450ms` (when over threshold)

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

Segments are Rust types that implement the `Segment` trait in `lynx-prompt`.
This section is for contributors who want to add a segment to the core.

### 1. Create the segment file

In `crates/lynx-prompt/src/segments/my_segment.rs`:

```rust
use lynx_theme::schema::SegmentConfig;
use crate::segment::{RenderContext, RenderedSegment, Segment};

pub struct MySegment;

impl Segment for MySegment {
    fn name(&self) -> &'static str {
        "my_segment"    // must match the key used in theme TOML [segment.*]
    }

    fn cache_key(&self) -> Option<&'static str> {
        None    // or Some("my_state") if you read from the cache
    }

    fn render(&self, config: &SegmentConfig, ctx: &RenderContext) -> Option<RenderedSegment> {
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
