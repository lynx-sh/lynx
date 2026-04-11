#!/usr/bin/env bats
# Integration tests for context detection and context-gating behavior

setup() {
  export HOME="$(mktemp -d)"
  export LYNX_TEST=1
}

teardown() {
  rm -rf "$HOME"
}

@test "default context is interactive" {
  unset CLAUDE_CODE CURSOR_SESSION CI LYNX_CONTEXT
  run lx init
  [ "$status" -eq 0 ]
  [[ "$output" == *"LYNX_CONTEXT=interactive"* ]]
}

@test "CLAUDE_CODE=1 triggers agent context" {
  CLAUDE_CODE=1 run lx init
  [ "$status" -eq 0 ]
  [[ "$output" == *"LYNX_CONTEXT=agent"* ]]
}

@test "CURSOR_SESSION set triggers agent context" {
  CURSOR_SESSION=abc run lx init
  [ "$status" -eq 0 ]
  [[ "$output" == *"LYNX_CONTEXT=agent"* ]]
}

@test "CI=true triggers minimal context" {
  CI=true run lx init
  [ "$status" -eq 0 ]
  [[ "$output" == *"LYNX_CONTEXT=minimal"* ]]
}

@test "LYNX_CONTEXT=minimal triggers minimal context" {
  LYNX_CONTEXT=minimal run lx init
  [ "$status" -eq 0 ]
  [[ "$output" == *"LYNX_CONTEXT=minimal"* ]]
}

@test "LYNX_CONTEXT=agent override wins over auto-detect" {
  LYNX_CONTEXT=agent run lx init
  [ "$status" -eq 0 ]
  [[ "$output" == *"LYNX_CONTEXT=agent"* ]]
}

@test "context status uses canonical detector (CLAUDE_CODE)" {
  run env CLAUDE_CODE=1 lx context status
  [ "$status" -eq 0 ]
  [[ "$output" == *"Detected:  agent (auto-detected agent (CLAUDE_CODE))"* ]]
}

@test "context status uses canonical detector (CI)" {
  run env CI=true lx context status
  [ "$status" -eq 0 ]
  [[ "$output" == *"Detected:  minimal (auto-detected minimal (CI))"* ]]
}

@test "explicit --context agent overrides detection" {
  run lx init --context agent
  [ "$status" -eq 0 ]
  [[ "$output" == *"LYNX_CONTEXT=agent"* ]]
}

@test "explicit --context interactive forces interactive" {
  CLAUDE_CODE=1 run lx init --context interactive
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
  run rg "CLAUDE_CODE|CURSOR_SESSION" "$src"
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
