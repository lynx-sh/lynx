# ADR-004: All Config is TOML — No zsh-as-Config

**Status:** Accepted

## Context

Shell frameworks typically store configuration as zsh code: users set variables,
call functions, and the framework sources the file. This makes config powerful
but also dangerous — a typo can produce syntax errors that break the shell.
Config cannot be read by other tools, validated ahead of time, or diffed easily.

The alternative — a structured config format — means the framework must provide
commands for every operation instead of letting users write arbitrary config.

## Decision

All Lynx configuration is TOML. There are no user-writable zsh config files.

- User config: `~/.config/lynx/config.toml`
- Plugin manifests: `plugin.toml` in each plugin directory
- Themes: `~/.config/lynx/themes/<name>.toml`
- Profiles: `~/.config/lynx/profiles/<name>.toml`

Config is read by Rust code (`lynx-config` crate) and validated against a typed
schema at load time. Validation errors are surfaced as human-readable messages,
not shell syntax errors.

Users modify config with `lx config set`, `lx theme switch`, `lx profile switch`,
and `lx plugin add/remove`. Direct file editing is supported but changes must
pass schema validation on next load.

## Consequences

**Positive:**
- Config can be validated before application — no "broken shell" from typos
- TOML files are diffs well and can be synced via git
- Config is readable by any tool that parses TOML
- Snapshot-before-mutate (D-007) is trivially implementable

**Negative:**
- Users cannot write arbitrary initialization code
- Every config capability must be explicitly implemented as a command
- Power users who want `~/.zshrc`-style programmatic config cannot do so

**Invariant this creates:**
- D-003: No JSON or YAML config — TOML only
- D-007: Every config mutation snapshots first, then validates, then applies
