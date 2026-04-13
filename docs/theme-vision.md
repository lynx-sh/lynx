# Lynx Theme System — Vision & Roadmap

This document captures the full intended design of the Lynx theme system.
It is the north star for all theme-related work. Read it before implementing
any theme feature. Implementation details live in `theme-authoring.md` and the
relevant crates. Decisions are in `pt decisions themes`.

---

## The Core Idea

A Lynx theme is not a prompt config. It is the **complete shell visual environment**.

When you switch a theme, your entire terminal changes coherently: the prompt
layout, the prompt character, file listing colors (`LS_COLORS`, `EZA_COLORS`),
and future surfaces like syntax highlighting and completion menus. One switch,
everything cohesive — no manual wiring.

This is what no other shell framework does. Starship and OMZ control only the
prompt. Users still manually set `LS_COLORS`, configure their colorizer, etc.
Lynx themes eliminate that.

---

## Why We're Better Than Starship / OMZ

| Concern | Starship / OMZ | Lynx |
|---|---|---|
| **Scope** | Prompt only | Full shell visual environment |
| **Color palette** | Per-segment hex values | `[colors]` palette, `$variable` refs in all configs |
| **Named colors** | Basic ANSI names | Rich registry: `light-blue`, `dark-red`, `orange`, etc., backed by curated hex — not ANSI index aliases |
| **Segment visibility** | Per-segment flags, inconsistent | Universal `show_in` / `hide_in` on every segment |
| **Segment config** | Shared config blocks (Starship) | Each segment owns its typed config |
| **Context awareness** | Limited or opt-in | First-class: interactive / agent / minimal auto-detected |
| **File listing colors** | User's problem | Defined in `[ls_colors]`, emitted on theme switch |
| **Composition** | OMZ: zsh functions (code). Starship: `format` strings | `format` strings + `custom_*` template segments — data not code |
| **Custom segments** | OMZ: write a zsh function. Starship: `[custom]` runs a shell command | `[segment.custom_NAME]` template over RenderContext — no I/O, no code |
| **CLI customization** | `starship config key value` (scalars only) | `lx theme patch` (any TOML path) + human shorthands + array/segment ops |
| **Visual theme builder** | None | `lx theme studio` — local WYSIWYG in the browser |
| **Safety** | No rollback | Snapshot → validate → rollback on every mutation |

### Themes are data, not code (D-024)

OMZ themes are zsh programs. Every theme is different code — nothing is
composable, toolable, or AI-editable. Starship moved to pure TOML. Lynx
goes further: TOML data with a **templating layer** gives the same power
as OMZ functions without the chaos.

Two mechanisms replace zsh functions entirely:

**1. Segment `format` strings (H-068)** — control how a segment's sub-values
are composed into output without touching Rust:
```toml
[segment.git_branch]
# default: just the branch name with icon
format = "$icon$branch"

# custom: wrap in brackets, show remote
format = "[$icon$branch → $remote]"
```
Each segment exposes named variables (`$branch`, `$icon`, `$status`, etc.).
The format string is a template, not code.

**2. Custom template segments (H-069)** — define a one-off segment entirely
in TOML using RenderContext data:
```toml
[segment.custom_greeting]
template = "[$env.USER@$env.HOSTNAME]"
color    = { fg = "$accent" }
show_in  = ["interactive"]

[segment.custom_clock]
template = "$time.hms"
color    = { fg = "$muted" }
```
Available template vars: `$env.USER`, `$env.HOSTNAME`, `$env.SSH_CONNECTION`,
`$cwd`, `$context`, `$cache.git_state.branch`, `$time.hms`, `$time.date`.
No I/O. No shell execution. Just data already in RenderContext.

---

## The Color Stack

Three layers for three audiences. All resolve to the same ANSI output:

```
Noob:        blue, light-blue, dark-red     ← named color registry (H-059)
Power user:  #7aa2f7                        ← hex, TrueColor with downgrade
Themer:      $accent in segment config      ← palette variable refs (H-051)
AI agent:    lx theme patch ... <any above> ← CLI mutation (H-060)
```

**Named colors are first-class (D-019).** `"blue"` on a TrueColor terminal
renders as a curated `#` hex value, not xterm index 4. The named registry
covers: `black`, `white`, `red`, `green`, `blue`, `yellow`, `magenta`, `cyan`,
`grey`, `orange`, `pink`, `purple`, and `light-*` / `dark-*` / `bright-*`
variants of each.

**Palette variables are canonical (D-015).** Hardcoded hex in segment configs
is a violation. All color values live in `[colors]`. Segments reference them
by name. The whole theme recolors by changing `[colors]` only.

Standard semantic palette keys (used by built-in segments and `[ls_colors]`):

| Key | Purpose |
|---|---|
| `accent` | Primary highlight (dirs, branches) |
| `success` | Clean state (prompt char, clean git) |
| `warning` | Dirty state (modified files, slow cmd) |
| `error` | Failure state (prompt char on non-zero exit, broken symlinks) |
| `muted` | Subdued text (timestamps, durations) |
| `fg` | Default foreground |
| `bg` | Default background (Powerline fill, separators) |

---

## The CLI Customization Surface

Three layers, same validated pipeline underneath. All write TOML, all snapshot
and rollback on failure (D-020):

### Shorthands — humans

```bash
lx theme caret "❯"
lx theme caret-color light-blue
lx theme palette accent dark-red
lx theme palette success "#9ece6a"

lx theme segment remove cmd_duration
lx theme segment move git_branch right
lx theme segment add venv left --after dir
```

### Patch — power users and AI agents

```bash
# Any scalar field by dot-path
lx theme patch colors.accent light-blue
lx theme patch segment.git_branch.icon "["
lx theme patch segment.git_status.staged.icon "✓"
lx theme patch segment.prompt_char.error_color.fg dark-red

# Visibility
lx theme patch segment.username.show_in '["interactive"]'
```

### Editor — full control

```bash
lx theme edit   # $EDITOR, validated on save, rollback on bad TOML
```

### Studio — WYSIWYG (H-062)

```bash
lx theme studio
```

Opens a local web UI in the browser. Live prompt preview, drag-and-drop
segment ordering, color pickers backed by the named color registry,
one-click apply. Ships embedded in the binary — no npm, no build step,
no external CDN (D-022). Output is identical TOML to hand-authored themes.

`lx theme studio` is the blessed tool for humans creating themes from scratch.
`lx theme patch` is for automation and quick surgical changes (D-021).

---

## Theme Schema — Full Target

The complete intended TOML schema for a Lynx theme. Fields marked *planned*
have a linked issue and are not yet implemented.

```toml
[meta]
name        = "my-theme"
description = "shown in lx theme list"
author      = "Your Name"

# ── Palette ─────────────────────────────────────────────────────────────
# All segment and ls_colors configs reference these by $name. (H-051)
[colors]
accent  = "#7aa2f7"
success = "#9ece6a"
warning = "#e0af68"
error   = "#f7768e"
muted   = "#565f89"
fg      = "#c0caf5"
bg      = "#1a1b26"

# ── Prompt layout ─────────────────────────────────────────────────────
[segments.left]
order = ["dir", "git_branch", "git_status", "prompt_char"]

[segments.right]
order = ["cmd_duration", "context_badge"]

# planned: segments.top (line above input), segments.continuation (PS2)
# tracked: H-058

# ── Separators (planned — H-057) ──────────────────────────────────────
# [separators]
# left  = { char = "", color = { fg = "$bg" } }   # Powerline glyph
# right = { char = "", color = { fg = "$bg" } }
# plain = " "                                       # fallback if no glyph font

# ── Segment config ────────────────────────────────────────────────────
# Every segment accepts: show_in, hide_in (D-017)
# Every segment owns its typed config — no shared flat struct (D-018)

[segment.dir]
max_depth        = 3
truncate_to_repo = true
color            = { fg = "$accent", bold = true }

[segment.git_branch]
icon  = " "
color = { fg = "$warning" }
# icon_end = ""   # optional closing bracket

[segment.git_status]
staged    = { icon = "+", color = { fg = "$success" } }
modified  = { icon = "!", color = { fg = "$warning" } }
untracked = { icon = "?", color = { fg = "$muted" } }

# planned: git_ahead_behind, git_stash — tracked: H-056
# [segment.git_ahead_behind]
# ahead_icon  = "⇡"
# behind_icon = "⇣"
# color       = { fg = "$muted" }

[segment.prompt_char]
symbol       = "❯"
error_symbol = "❯"
color        = { fg = "$success" }
error_color  = { fg = "$error" }
# tracked: H-055

[segment.cmd_duration]
min_ms = 500
color  = { fg = "$muted" }

[segment.context_badge]
show_in = ["agent", "minimal"]
label   = { agent = "AI", minimal = "MIN" }
color   = { fg = "$accent", bold = true }

[segment.kubectl_context]
prod_pattern = "prod-.*"
color        = { fg = "$warning" }
hide_in      = ["minimal"]

# planned segments (H-056):
# username, hostname, exit_code, venv, newline,
# git_ahead_behind, git_stash

# ── File listing colors (planned — H-054) ────────────────────────────
# Emitted as LS_COLORS + EZA_COLORS on theme switch / shell init.
# [ls_colors]
# dir        = { fg = "$accent", bold = true }
# symlink    = { fg = "light-blue" }
# executable = { fg = "$success", bold = true }
# archive    = { fg = "$warning" }
# image      = { fg = "pink" }
# audio      = { fg = "pink" }
# broken     = { fg = "$error" }
```

---

## Implementation Roadmap

Priority order. Each issue has full context in `pt issue H-XXX`.

### P1 — Foundation (must land before P2 work)

| Issue | What | Why it unlocks |
|---|---|---|
| H-059 | Named color registry with curated hex backing | Makes named colors usable everywhere |
| H-051 | Palette `$variable` resolution in segment configs | Single-point theming |
| H-053 | Universal `show_in` / `hide_in` on every segment | Visibility without Rust changes |
| H-052 | Typed per-segment config (kill flat `SegmentConfig`) | Scalable segment addition |
| H-054 | `[ls_colors]` emitted as `LS_COLORS` / `EZA_COLORS` | Full environment ownership |
| H-060 | `lx theme patch` CLI with scalar mutation | Power user / AI interface |
| H-061 | Array mutation + `lx theme segment` shorthands | Structural CLI customization |
| H-068 | Segment `format` strings — compose sub-values in TOML | Replaces OMZ zsh functions for layout |
| H-069 | `custom_*` template segments over RenderContext | Replaces OMZ zsh functions for custom output |

### P2 — Completeness

| Issue | What |
|---|---|
| H-055 | `prompt_char` segment (themeable caret, exit-code coloring) |
| H-056 | Missing segments: username, hostname, exit_code, venv, newline, git ahead/behind/stash |
| H-057 | Segment separators / Powerline connector config |
| H-062 | `lx theme studio` WYSIWYG local web UI |

### P3 — Polish

| Issue | What |
|---|---|
| H-058 | Multi-line prompt layout, PS2 continuation, transient prompt |

---

## Decisions (authoritative source: `pt decisions themes`)

| ID | Rule |
|---|---|
| D-015 | Palette vars are canonical — no hardcoded hex in segment configs |
| D-016 | Theme owns the full shell visual environment, not just the prompt |
| D-017 | Universal `show_in` / `hide_in` on every segment — no ad-hoc per-segment logic |
| D-018 | Segments own typed config — no shared flat `SegmentConfig` struct |
| D-019 | Named colors are backed by curated hex, not ANSI index aliases |
| D-020 | `lx theme patch` is the AI/automation interface; shorthands are for humans |
| D-021 | `lx theme studio` is the blessed human authoring tool; patch is for automation |
| D-022 | Studio frontend is a single embedded HTML file — no npm, no build step |
| D-024 | Themes are data not code — format strings and `custom_*` templates, never executable zsh |
