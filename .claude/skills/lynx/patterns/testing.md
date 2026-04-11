# Testing Protocol

## Test Types and Where They Live

| Type | Tool | Location | When Required |
|---|---|---|---|
| Unit tests | cargo nextest | crates/<name>/src/ (inline) | Every new fn with logic |
| Integration (Rust) | cargo nextest | tests/integration/rust/ | Cross-crate behavior |
| Shell integration | bats | tests/integration/shell/ | Anything in shell/ |
| Theme tests | cargo nextest | crates/lynx-theme/src/ | Every theme field change |
| Guardrail tests | bats + script | tests/integration/shell/test_*.bats + scripts/verify-guardrails.sh | When a new architectural invariant is added or a new drift class is found |

## Test Isolation Rule

**No test may touch the real user $HOME.**

Use `lynx_test_utils::temp_home()` in Rust tests.
Use `setup() { export HOME="$(mktemp -d)"; }` in bats tests.

Every test is responsible for its own cleanup. No shared fixtures that persist between tests.

## Running Tests

```bash
# All Rust tests
cargo nextest run --all

# Single crate
cargo nextest run -p lynx-<name>

# Specific test
cargo nextest run -p lynx-<name> -E 'test(<test_name>)'

# Shell integration
bats tests/integration/shell/

# Syntax check any zsh output
echo "$zsh_output" | zsh -n

# ALL invariant guardrails (run before every pt fix / session end)
scripts/verify-guardrails.sh
```

## Guardrail Tests

`scripts/verify-guardrails.sh` is the unified offline verifier. It runs 38 checks across 5 drift classes:

| Class | What it checks |
|---|---|
| Shell protocol | Line limits (60 for core, 10 for plugin init.zsh), no branching in static files |
| Context mismatch | `CLAUDE_CODE`, `CURSOR_SESSION`, `CI` constants still in `lynx-shell/src/context.rs` |
| Dep map drift | Forbidden crate dep pairs from `maps/crate-deps.md` (lynx-core↛lynx-*, etc.) |
| Checksum enforcement | `validate_index` fn exists and is called in `fetch_plugin` |
| Docs-command mismatch | Critical `lx` commands documented in README; subcommands declared in `cli.rs` |

**Guardrail tests live in:**
- `tests/integration/shell/test_init.bats` — shell protocol class
- `tests/integration/shell/test_context.bats` — context mismatch class
- `tests/integration/shell/test_doctor.bats` — dep map, checksum, docs-command classes

**When to add a guardrail test:**
1. A new architectural invariant is established (new decision D-XXX)
2. A regression from a known drift class is found and fixed
3. A new forbidden dep pair is added to `maps/crate-deps.md`
4. A new critical CLI command is added that must stay documented

**When NOT to add a guardrail test:** don't add guardrails for behavior already covered by unit or integration tests. Guardrails catch *structural drift* (wrong patterns in source), not behavioral bugs.

## Shell Test Pattern (bats)

```bash
setup() {
  export HOME="$(mktemp -d)"
  export LYNX_TEST=1
  # copy test fixtures if needed
}

teardown() {
  rm -rf "$HOME"
}

@test "description of what is verified" {
  run lx <command>
  [ "$status" -eq 0 ]
  [[ "$output" == *"expected_substring"* ]]
}
```

## Theme Tests

When adding or modifying theme fields:
1. Parse the theme file in a test — verify no panic
2. Verify unknown segment names produce a warning, not an error
3. Verify color downgrade works at each TermCapability level
4. Test: missing field falls back to theme default, not Rust default

## What "Tests Written" Means in a Gate

- At minimum: one test per new public function
- Test must be in the same PR/commit as the code
- "impossible because X" must be a genuine reason (e.g. "tests shell subprocess behavior — covered by bats")
- Untested code that causes a regression = P0
