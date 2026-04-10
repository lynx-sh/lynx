# Crate Implementation Protocol

## Before Creating a New Crate

1. Check `maps/crate-deps.md` — where does it sit in the tree?
2. Does it belong in an existing crate? (prefer extending over adding)
3. Run GATE: CRATE before writing any files

## File Organization Rules

**One responsibility per file.** If a file name would be `utils.rs` or `helpers.rs` — wrong.
Name files by what they do: `loader.rs`, `validator.rs`, `renderer.rs`.

**No god files.** If any file exceeds 300 lines, split it. The split must be by logical domain,
not arbitrary line count.

**Module structure:**
```
crates/lynx-<name>/
├── Cargo.toml
└── src/
    ├── lib.rs          # pub mod declarations only — no logic
    ├── <domain1>.rs    # one domain per file
    ├── <domain2>.rs
    └── <subdomain>/    # subdirectory only if >3 related files
        ├── mod.rs
        └── *.rs
```

`lib.rs` contains ONLY `pub mod` declarations and re-exports. Zero logic.

## Error Handling Rules

- All errors use `LynxError` from `lynx-core::error`
- No `unwrap()` outside of tests
- No `expect()` with a Rust-internal message — use `expect("human-readable context")`
- `?` propagation is preferred over match on errors
- User-facing errors must pass through `redact()` if they may contain config values

## Naming Conventions

```rust
// Structs: PascalCase, noun
pub struct PluginManifest { ... }

// Functions: snake_case, verb
pub fn load_plugin(name: &str) -> Result<PluginManifest> { ... }

// Constants: SCREAMING_SNAKE_CASE
pub const MAX_PLUGIN_LOAD_MS: u64 = 500;

// Error variants: PascalCase, descriptive noun/adjective
LynxError::ManifestNotFound(String)
LynxError::CircularDependency(Vec<String>)

// Module names: snake_case, domain noun
mod dep_graph;
mod lifecycle;
mod namespace;
```

## Cargo.toml Rules

- Use `workspace.dependencies` for all shared deps — no version pinning in individual crates
- No `[dev-dependencies]` that duplicate `[dependencies]`
- `lynx-test-utils` must be in `[dev-dependencies]` only, never `[dependencies]`
- Features must be documented in a comment above `[features]`

## What lynx-cli Is and Is NOT

`lynx-cli` is the assembler. It:
- Parses CLI args (clap)
- Calls into other crates
- Formats output for the user

It does NOT:
- Contain business logic
- Contain types used by other crates
- Import from itself recursively

If you find business logic in `lynx-cli/src/commands/`, move it to the appropriate crate.
