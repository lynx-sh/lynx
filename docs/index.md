# Lynx Documentation

Fast, Rust-powered zsh shell framework. Context-aware, plugin-isolated, theme-driven.

## Core Architecture

| Document | Audience | What it covers |
|---|---|---|
| [Architecture](architecture.md) | Contributors, agents | Crate map, shell integration flow, plugin lifecycle, event system |
| [Plugin Authoring](plugin-authoring.md) | Plugin developers | Building, testing, and publishing plugins |
| [Plugin API](plugin-api.md) | Plugin developers | Plugin manifest, exports, hooks, ZLE widgets |

## Theme System

| Document | Audience | What it covers |
|---|---|---|
| [Theme Vision](theme-vision.md) | Contributors, agents | Full theme system design, decisions summary |
| [Theme Authoring](theme-authoring.md) | Theme developers | Theme TOML schema, segments, separators, colors |

## Ecosystem & Registry

| Document | Audience | What it covers |
|---|---|---|
| [Ecosystem](ecosystem.md) | Everyone | Taps, package types, trust tiers, `lx install`, community taps |
| [Registry Index Spec](registry-index-spec.md) | Registry maintainers | Index TOML format for taps and the official registry |

## Configuration & Sync

| Document | Audience | What it covers |
|---|---|---|
| [Config & Sync](config-and-sync.md) | Users | `lx config` subcommands, settable keys, `lx sync` git-backed sync |

## Workflow Engine

| Document | Audience | What it covers |
|---|---|---|
| [Workflows](workflows.md) | Users, contributors | Workflow TOML schema, runners, `lx run`, jobs, cron |

## Dashboard

| Document | Audience | What it covers |
|---|---|---|
| [Dashboard](dashboard.md) | Contributors, agents | Web UI architecture, API surface, frontend design |

## Decisions

Architecture decisions are tracked in `pt` (the project tracker):

```bash
pt decisions              # list all active decisions
pt decisions arch         # architecture decisions
pt decisions themes       # theme decisions
pt decisions registry     # ecosystem/registry decisions
pt decisions cli          # CLI decisions
```

## Troubleshooting

| Document | Audience | What it covers |
|---|---|---|
| [Troubleshooting](troubleshooting.md) | Users | Shell startup errors, eval failures, `lx doctor`, diagnostic tools |

## Quick Links

- [README](../README.md) — install and quickstart
- [CONTRIBUTING](../CONTRIBUTING.md) — how to contribute
- [GitHub](https://github.com/lynx-sh/lynx) — source code
- [Registry](https://github.com/lynx-sh/registry) — official package registry
