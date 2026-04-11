# Lynx Documentation

Fast, Rust-powered zsh shell framework. Context-aware, plugin-isolated, theme-driven.

## Guides

| Document | Audience | What it covers |
|---|---|---|
| [Architecture](architecture.md) | Contributors, advanced users | Crate map, shell integration flow, plugin lifecycle, event system |
| [Plugin Authoring](plugin-authoring.md) | Plugin developers | Building, testing, and publishing plugins |
| [Theme Authoring](theme-authoring.md) | Theme developers | Theme TOML, all segments, custom segments in Rust |
| [Registry Index Spec](registry-index-spec.md) | Registry maintainers | Static index format for the plugin registry |

## Decisions (ADRs)

Architecture decisions are recorded in `docs/decisions/`. These explain *why* key design choices were made — the implementation details are in the code.

| ADR | Decision |
|---|---|
| [ADR-001](decisions/adr-001-rust-thin-zsh.md) | Rust core with thin zsh eval-bridge |
| [ADR-002](decisions/adr-002-drop-omz-compat.md) | No OMZ compatibility layer |
| [ADR-003](decisions/adr-003-agent-context-detection.md) | Automatic agent context detection via env vars |
| [ADR-004](decisions/adr-004-toml-everywhere.md) | All config is TOML — no zsh-as-config |
| [ADR-005](decisions/adr-005-git-backed-sync.md) | Git-backed config sync |

## Quick links

- [README](../README.md) — install and quickstart
- [CONTRIBUTING](../CONTRIBUTING.md) — how to contribute
