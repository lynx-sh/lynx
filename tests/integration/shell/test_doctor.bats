#!/usr/bin/env bats
# Integration tests for lx doctor

setup() {
  export HOME="$(mktemp -d)"
  export LYNX_TEST=1
  # Minimal config for a clean doctor pass
  mkdir -p "$HOME/.config/lynx"
  cat > "$HOME/.config/lynx/config.toml" << 'TOML'
schema_version = 1
enabled_plugins = []
active_theme = "default"
active_context = "interactive"
TOML
}

teardown() {
  rm -rf "$HOME"
}

@test "lx doctor exits 0 on clean config" {
  run lx doctor
  [ "$status" -eq 0 ]
}

@test "lx doctor output does not contain ERROR on clean config" {
  run lx doctor
  [[ "$output" != *"ERROR"* ]]
}

@test "config mutators use guarded transaction helper (no direct save paths)" {
  local files=(
    "$BATS_TEST_DIRNAME/../../../crates/lynx-cli/src/commands/config.rs"
    "$BATS_TEST_DIRNAME/../../../crates/lynx-cli/src/commands/plugin"
    "$BATS_TEST_DIRNAME/../../../crates/lynx-cli/src/commands/profile.rs"
    "$BATS_TEST_DIRNAME/../../../crates/lynx-cli/src/commands/theme.rs"
    "$BATS_TEST_DIRNAME/../../../crates/lynx-cli/src/commands/context.rs"
    "$BATS_TEST_DIRNAME/../../../crates/lynx-cli/src/commands/sync.rs"
    "$BATS_TEST_DIRNAME/../../../crates/lynx-cli/src/commands/migrate.rs"
  )

  run rg -rn "\\bsave\\(|\\bsave_config\\(|\\bsave_to\\(" "${files[@]}"
  [ "$status" -eq 1 ]
}

# ── Guardrail: dependency map drift ──────────────────────────────────────────

@test "guardrail: lynx-core has no internal lynx-* dependencies" {
  local cargo="$BATS_TEST_DIRNAME/../../../crates/lynx-core/Cargo.toml"
  # lynx-core must not depend on any other lynx-* crate
  run rg "lynx-" "$cargo"
  [ "$status" -eq 1 ]
}

@test "guardrail: lynx-prompt does not depend on lynx-loader (circular)" {
  local cargo="$BATS_TEST_DIRNAME/../../../crates/lynx-prompt/Cargo.toml"
  run rg "lynx-loader" "$cargo"
  [ "$status" -eq 1 ]
}

@test "guardrail: lynx-events does not depend on lynx-plugin (circular)" {
  local cargo="$BATS_TEST_DIRNAME/../../../crates/lynx-events/Cargo.toml"
  run rg "lynx-plugin" "$cargo"
  [ "$status" -eq 1 ]
}

@test "guardrail: lynx-shell does not depend on lynx-cli" {
  local cargo="$BATS_TEST_DIRNAME/../../../crates/lynx-shell/Cargo.toml"
  run rg "lynx-cli" "$cargo"
  [ "$status" -eq 1 ]
}

@test "guardrail: no crate except lynx-cli depends on lynx-cli" {
  local crates_dir="$BATS_TEST_DIRNAME/../../../crates"
  # Search all Cargo.toml files outside of lynx-cli itself
  run bash -c "
    find '$crates_dir' -name 'Cargo.toml' \
      ! -path '*/lynx-cli/*' \
      -exec rg -l 'lynx-cli' {} +
  "
  [ "$status" -eq 1 ] || [ -z "$output" ]
}

# ── Guardrail: docs-command mismatch ─────────────────────────────────────────

@test "guardrail: critical CLI commands documented in README" {
  local readme="$BATS_TEST_DIRNAME/../../../README.md"
  # These are the user-facing commands that must remain documented
  for cmd in "lx doctor" "lx plugin" "lx theme" "lx context" "lx init"; do
    run rg "$cmd" "$readme"
    [ "$status" -eq 0 ]
  done
}

@test "guardrail: CLI subcommand list matches cli.rs declarations" {
  local cli_rs="$BATS_TEST_DIRNAME/../../../crates/lynx-cli/src/cli.rs"
  # Verify the canonical subcommands are present in cli.rs
  for cmd in Init Plugin Theme Context Doctor Daemon Config Migrate Update Rollback Sync; do
    run rg "\\b${cmd}\\b" "$cli_rs"
    [ "$status" -eq 0 ]
  done
}

# ── Guardrail: index/lock checksum enforcement ────────────────────────────────

@test "guardrail: validate_index fn exists in lynx-registry" {
  local src="$BATS_TEST_DIRNAME/../../../crates/lynx-registry/src/index.rs"
  run rg "fn validate_index" "$src"
  [ "$status" -eq 0 ]
}

@test "guardrail: fetch pipeline calls validate_index before installing" {
  local fetch="$BATS_TEST_DIRNAME/../../../crates/lynx-registry/src/fetch.rs"
  run rg "validate_index" "$fetch"
  [ "$status" -eq 0 ]
}
