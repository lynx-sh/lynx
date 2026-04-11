#!/usr/bin/env bats
# Integration tests for context detection and context-gating behavior

load helpers

setup() {
  export HOME="$(mktemp -d)"
  export LYNX_TEST=1
}

teardown() {
  rm -rf "$HOME"
}

@test "default context is interactive" {
  unset "$LYNX_VAR_CLAUDECODE" "$LYNX_VAR_CURSOR_CLI" "$LYNX_VAR_CI" "$LYNX_VAR_CONTEXT"
  run lx init
  [ "$status" -eq 0 ]
  [[ "$output" == *"LYNX_CONTEXT=interactive"* ]]
}

@test "CLAUDECODE=1 triggers agent context" {
  eval "$LYNX_VAR_CLAUDECODE=1" run lx init
  [ "$status" -eq 0 ]
  [[ "$output" == *"LYNX_CONTEXT=agent"* ]]
}

@test "CURSOR_CLI set triggers agent context" {
  eval "$LYNX_VAR_CURSOR_CLI=abc" run lx init
  [ "$status" -eq 0 ]
  [[ "$output" == *"LYNX_CONTEXT=agent"* ]]
}

@test "CI=true triggers minimal context" {
  run env -u "$LYNX_VAR_CLAUDECODE" -u "$LYNX_VAR_CURSOR_CLI" -u "$LYNX_VAR_CONTEXT" CI=true lx init
  [ "$status" -eq 0 ]
  [[ "$output" == *"LYNX_CONTEXT=minimal"* ]]
}

@test "LYNX_CONTEXT=minimal triggers minimal context" {
  run env -u "$LYNX_VAR_CLAUDECODE" -u "$LYNX_VAR_CURSOR_CLI" LYNX_CONTEXT=minimal lx init
  [ "$status" -eq 0 ]
  [[ "$output" == *"LYNX_CONTEXT=minimal"* ]]
}

@test "LYNX_CONTEXT=agent override wins over auto-detect" {
  LYNX_CONTEXT=agent run lx init
  [ "$status" -eq 0 ]
  [[ "$output" == *"LYNX_CONTEXT=agent"* ]]
}

@test "context status uses canonical detector (CLAUDECODE)" {
  run env "$LYNX_VAR_CLAUDECODE=1" lx context status
  [ "$status" -eq 0 ]
  [[ "$output" == *"Detected:  agent (auto-detected agent (CLAUDECODE))"* ]]
}

@test "context status uses canonical detector (CI)" {
  run env -u "$LYNX_VAR_CLAUDECODE" -u "$LYNX_VAR_CURSOR_CLI" -u "$LYNX_VAR_CONTEXT" CI=true lx context status
  [ "$status" -eq 0 ]
  [[ "$output" == *"Detected:  minimal (auto-detected minimal (CI))"* ]]
}

@test "explicit --context agent overrides detection" {
  run lx init --context agent
  [ "$status" -eq 0 ]
  [[ "$output" == *"LYNX_CONTEXT=agent"* ]]
}

@test "explicit --context interactive forces interactive" {
  eval "$LYNX_VAR_CLAUDECODE=1" run lx init --context interactive
  [ "$status" -eq 0 ]
  [[ "$output" == *"LYNX_CONTEXT=interactive"* ]]
}

@test "lx init output is valid zsh in agent context" {
  run lx init --context agent
  [ "$status" -eq 0 ]
  echo "$output" | zsh -n
}

@test "lx init output is valid zsh in minimal context" {
  run lx init --context minimal
  [ "$status" -eq 0 ]
  echo "$output" | zsh -n
}

# ── Guardrail: context mismatch ───────────────────────────────────────────────

@test "guardrail: context detector source references canonical env vars" {
  local src="$BATS_TEST_DIRNAME/../../../crates/lynx-shell/src/context.rs"
  run rg "$LYNX_VAR_CLAUDECODE|$LYNX_VAR_CURSOR_CLI" "$src"
  [ "$status" -eq 0 ]
}

@test "guardrail: CI env var listed in MINIMAL_ENV_VARS in context source" {
  local src="$BATS_TEST_DIRNAME/../../../crates/lynx-shell/src/context.rs"
  run rg "MINIMAL_ENV_VARS" "$src"
  [ "$status" -eq 0 ]
}

@test "guardrail: three valid context variants exist in lynx-core types" {
  local types="$BATS_TEST_DIRNAME/../../../crates/lynx-core/src/types.rs"
  run rg "Interactive" "$types"
  [ "$status" -eq 0 ]
  run rg "Agent" "$types"
  [ "$status" -eq 0 ]
  run rg "Minimal" "$types"
  [ "$status" -eq 0 ]
}
