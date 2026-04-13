# Testing — Lynx

## Rules
- **Never `run_in_background` for cargo commands** — background produces empty output.
- **Final verification: `lx run lynx-ai-verify`** — once per session, at the end. No baseline runs.
- **During work: targeted only** — `cargo nextest run -p lynx-<crate>` for the crate you changed.

## During Work

```bash
cargo nextest run -p lynx-prompt                      # crate under change
cargo nextest run -p lynx-prompt -E 'test(assemble)'  # single test/module
bats tests/integration/shell/                         # after any shell/ change
```

## Before Touching Code

Run the targeted test(s) for the area you're changing. If they fail before you start: stop, alert the architect.

## Final Verification (once, at task end)

```bash
lx run lynx-ai-verify   # clippy + full suite — errors only
```

Fix any failure before closing. Do not close the task with a red gate.

## Test Locations

| Type | Location | Runner |
|------|----------|--------|
| Rust unit/integration | `crates/lynx-*/src/**` (inline `#[cfg(test)]`) | `cargo nextest run -p lynx-<crate>` |
| Shell integration | `tests/integration/shell/` | `bats tests/integration/shell/` |

## Writing Tests

- Tests live in `#[cfg(test)] mod tests` at the bottom of the file they test
- Use descriptive snake_case names
- Use `lynx-test-utils` for shared helpers — never duplicate test infrastructure
