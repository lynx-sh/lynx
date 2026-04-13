# Plugin Protocol — Lynx

## Before Touching Plugin Code

```bash
pt decisions plugins    # plugin rules
pt decisions arch       # D-008: EventBus is the only cross-plugin channel
```

## Plugin Structure

Every plugin lives in `plugins/<name>/` with:
- `plugin.toml` — manifest (name, version, context gates, dependencies)
- `shell/init.zsh` — shell-side init (sourced by lx, logic-free)
- Optional: `shell/hooks.zsh`, `shell/aliases.zsh`

## Rules

1. **Plugins communicate only through EventBus** (D-008) — no direct imports between plugins
2. **Aliases are context-gated** — never unconditional. Use the `contexts` field in `plugin.toml`
3. **Language/cloud segments are plugins** (D-023) — not core segments
4. **Shell files are logic-free** — they source what `lx` tells them to. Computation happens in Rust

## Adding a Plugin

1. Create `plugins/<name>/plugin.toml` with required fields
2. Create `shell/init.zsh` (minimal — eval bridge only)
3. Add tests in `crates/lynx-plugin/` for manifest parsing
4. Run `pt wiring` — connect the plugin to the loader

## Verification

```bash
cargo nextest run -p lynx-plugin
cargo nextest run -p lynx-cli
```
