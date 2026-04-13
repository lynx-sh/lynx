# Crate Protocol

## Dependency Direction
- `lynx-core` → nothing internal (foundation)
- `lynx-config` → `lynx-core` only
- `lynx-cli` → everything (leaf — nothing depends on it)
- No cycles. Workspace versions only (`Cargo.toml` `[workspace.dependencies]`).

## Before Adding a Crate or Dep
```bash
pt decisions arch
cargo build -p lynx-<new-crate>   # confirms no dep cycle
grep "<dep>" Cargo.toml           # must exist in [workspace.dependencies]
```

## New Crate Checklist
1. Add to `[workspace.members]` in root `Cargo.toml`
2. `[package]` uses `workspace = true` for version/edition/authors
3. File `pt add` if wiring needed in another crate

## File Organization (D-042)
One domain per file. Before creating: does a file for this domain exist? Add to it. Can you describe the file without "and"? Good. Would an existing file gain a second responsibility? Split it first.
