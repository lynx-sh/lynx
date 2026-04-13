# Lynx Ecosystem Architecture

## Overview

Lynx is a shell platform with a package ecosystem. Users install tools, plugins, themes,
intros, and bundles through a unified interface (`lx install`). Packages are distributed
via **taps** — Git repositories containing a registry index.

## Related Decisions

Run `pt decisions registry` and `pt decisions distribution` for the full list.

## Package Types

| Type     | What it is                              | Where it lives        | Install method             |
|----------|-----------------------------------------|-----------------------|----------------------------|
| plugin   | Lynx plugin (plugin.toml + shell/)      | Registry or tap       | Fetch archive, extract     |
| tool     | System binary (eza, bat, fd, etc.)      | User's package manager| brew/apt/cargo/url install  |
| theme    | Theme TOML file                         | Registry or tap       | Download TOML to themes/   |
| intro    | Intro TOML file                         | Registry or tap       | Download TOML to intros/   |
| bundle   | Collection of other packages            | Registry or tap       | Recursive install          |

## Tap System

A **tap** is a Git repository containing a `registry.toml` (same schema as the official
index). Anyone can create a tap — no approval required.

### Trust Tiers

| Tier       | Badge | Meaning                                  |
|------------|-------|------------------------------------------|
| Official   | ✓     | Curated by Lynx maintainers              |
| Verified   | ◆     | Passes automated validation              |
| Community  | ○     | Unreviewed — user warned before install   |

### Tap Management

| Command                   | Description                                              |
|---------------------------|----------------------------------------------------------|
| `lx tap list`             | Browse all configured taps with name, URL, and trust tier |
| `lx tap add <source>`     | Register a new tap by GitHub shorthand or full URL       |
| `lx tap remove <name>`    | Remove a tap (official tap cannot be removed)            |
| `lx tap update`           | Fetch all tap indexes to local cache                     |

**Adding a tap**

```bash
lx tap add community/lynx-taps              # GitHub shorthand — name becomes "community/lynx-taps"
lx tap add https://github.com/user/repo/raw/main/index.toml  # full URL — name becomes "user/repo"
```

After adding a tap, run `lx tap update` before packages from it appear in `lx browse` or `lx install`.

Name derivation: GitHub URLs (`github.com` or `raw.githubusercontent.com`) extract `user/repo`
from the path. Other URLs use `hostname/last-segment`.

Trust tiers apply to all tap packages — see [Trust Tiers](#trust-tiers) above.

### Creating a Community Tap

Create a GitHub repo with this structure:

```
my-tap/
├── registry.toml        # package index (same schema as official)
├── plugins/             # optional — plugin source archives
├── themes/              # optional — theme TOML files
└── intros/              # optional — intro TOML files
```

Users add it with `lx tap add yourname/my-tap`. No PR to the official registry needed.

## Package Installation

```bash
lx install eza                     # tool: detects brew/apt, installs, creates plugin
lx install syntax-highlight        # plugin: enables bundled plugin
lx install catppuccin              # theme: downloads TOML to themes/
lx install modern-cli              # bundle: installs eza + bat + fd + ripgrep + zoxide
lx uninstall eza                   # removes Lynx integration, keeps system binary
```

### Tool Installation Flow

1. Resolve package from all taps (highest trust tier wins)
2. Detect user's package manager (brew → apt → dnf → pacman → cargo)
3. Show trust tier warning for community packages
4. Run install command with user confirmation
5. Auto-generate a Lynx plugin with context-gated aliases
6. Enable the plugin in config.toml

### Safety

- Community packages show a warning before install
- `lx uninstall` never removes system binaries — only Lynx integration
- `lx audit` shows what each package exports, hooks, and accesses
- Checksums verified for all archive downloads
- Config mutations always snapshot first (see `pt decisions config`)

## Repository Structure

```
lynx-sh/lynx             # framework source code
lynx-sh/registry          # official tap: index.toml + themes + intros
lynx-sh/homebrew-lynx     # Homebrew formula
```

## Browsing

```bash
lx browse                          # all packages by category
lx browse security                 # filter by category
lx browse --type tool              # filter by type
lx browse --installed              # show only installed
lx plugin search "git"             # search across all taps
lx plugin info eza                 # detailed package info
```
