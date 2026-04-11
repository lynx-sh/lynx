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
  unset CLAUDE_CODE CURSOR_SESSION CODEIUM_SESSION COPILOT_AGENT WINDSURF_AGENT CI LYNX_CONTEXT
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

@test "WINDSURF_AGENT set triggers agent context" {
  WINDSURF_AGENT=1 run lx init
  [ "$status" -eq 0 ]
  [[ "$output" == *"LYNX_CONTEXT=agent"* ]]
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
