# Testing Protocol

## Test Types and Where They Live

| Type | Tool | Location | When Required |
|---|---|---|---|
| Unit tests | cargo nextest | crates/<name>/src/ (inline) | Every new fn with logic |
| Integration (Rust) | cargo nextest | tests/integration/rust/ | Cross-crate behavior |
| Shell integration | bats | tests/integration/shell/ | Anything in shell/ |
| Theme tests | cargo nextest | crates/lynx-theme/src/ | Every theme field change |

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
```

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
