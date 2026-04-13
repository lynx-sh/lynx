# Testing — Lynx

## Universal Rules
- **NEVER `run_in_background` for `cargo nextest` or `cargo build`** — background produces empty output, wastes tokens re-running.
- **Full suite (`cargo nextest run --all`) runs ONCE per session** — at the very end, as final verification. No "baseline" full runs.
- **During work: targeted tests only** — `cargo nextest run -p lynx-<crate>` for the crate you changed.

## During Work — Targeted Tests

```bash
# Test the crate you're changing
cargo nextest run -p lynx-prompt

# Test a specific function/module
cargo nextest run -p lynx-prompt -E 'test(assemble)'

# After touching shell integration
bats tests/integration/shell/
```

Always foreground, `timeout: 300000`.

## Before Touching Code

Run the targeted test(s) for the area you're about to change. If they fail before you start: stop, alert the architect.

## Final Verification (task end, once)

```bash
cargo nextest run --all          # all crates
cargo clippy --all               # zero warnings policy
```

If either fails: fix before closing. Do not close the task.

## Test Locations

| Type | Location | Runner |
|------|----------|--------|
| Rust unit/integration | `crates/lynx-*/src/**` (inline `#[cfg(test)]`) | `cargo nextest run -p lynx-<crate>` |
| Shell integration | `tests/integration/shell/` | `bats tests/integration/shell/` |

## Writing Tests

- Tests live in `#[cfg(test)] mod tests` at the bottom of the file they test
- Test names: `fn test_<what>_<case>()` or descriptive snake_case
- Use `lynx-test-utils` for shared test helpers — never duplicate test infrastructure
