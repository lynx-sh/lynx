# Crate Protocol — Lynx

## Before Adding a Crate or Cross-Crate Dependency

```bash
pt decisions arch      # check for architectural constraints
```

### Dependency Direction Rules
- `lynx-core` depends on nothing internal — it's the foundation
- `lynx-config` depends on `lynx-core` only
- `lynx-cli` is the leaf — depends on everything, nothing depends on it
- No circular dependencies — ever
- New dependencies must use workspace versions from root `Cargo.toml`

### Verify Before Adding
```bash
# Check the dep doesn't create a cycle
cargo build -p lynx-<new-crate>

# Check workspace version exists
grep "<dep-name>" Cargo.toml   # must exist in [workspace.dependencies]
```

If the workspace dependency doesn't exist, add it to root `Cargo.toml` `[workspace.dependencies]` first.

## File Organization (D-042)

Each file owns one logical domain. Before creating a new file:
1. Does this domain already have a file? → Add to it
2. Can you describe the file without "and"? → Good, create it
3. Would an existing file gain a second responsibility? → Split the existing file first

## New Crate Checklist

1. Add to `[workspace.members]` in root `Cargo.toml`
2. Use `[package]` with `workspace = true` for version, edition, authors
3. Add to any CI/test scripts that enumerate crates
4. File a `pt add` if wiring is needed (integration point in another crate)
