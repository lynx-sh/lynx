# Plugin Protocol

## Rules
1. Plugins communicate only through EventBus (D-008) — no direct cross-plugin imports
2. Aliases are context-gated — use `contexts` field in `plugin.toml`, never unconditional
3. Language/cloud segments are plugins (D-023) — not core
4. Shell files are logic-free — eval bridge only, computation in Rust

## Adding a Plugin
1. `plugins/<name>/plugin.toml` with required fields
2. `shell/init.zsh` — eval bridge only
3. Tests in `crates/lynx-plugin/` for manifest parsing
4. `pt wiring` — connect to the loader

## Verify
```bash
cargo nextest run -p lynx-plugin
cargo nextest run -p lynx-cli
```
