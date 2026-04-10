# ADR-004: TOML for All Configuration

**Status:** Accepted

## Context
OMZ uses raw zsh files for themes (fragile, executable code as config). Starship uses TOML successfully. YAML is whitespace-sensitive and has footguns. JSON has no comments.

## Decision
All Lynx config (user config, plugin manifests, themes, tasks, contexts, profiles) uses TOML. It is human-readable, has comments, is typed, and pairs naturally with Rust's serde ecosystem.

## Consequences
- Consistent config experience across all subsystems.
- Themes are data, not code — can't break your shell.
- Rust parsing via serde_toml is fast and produces excellent error messages.
