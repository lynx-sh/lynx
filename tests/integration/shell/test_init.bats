#!/usr/bin/env bats
# Integration tests for lx init and shell integration layer
# Each test runs in an isolated temp HOME directory

setup() {
  export HOME="$(mktemp -d)"
  export LYNX_TEST=1
}

teardown() {
  rm -rf "$HOME"
}

@test "lx init produces valid zsh output" {
  run lx init --context interactive
  [ "$status" -eq 0 ]
  echo "$output" | zsh -n   # syntax check
}

@test "lx init sets LYNX_CONTEXT in output" {
  run lx init --context agent
  [ "$status" -eq 0 ]
  [[ "$output" == *"LYNX_CONTEXT=agent"* ]]
}

@test "agent context detected from CLAUDE_CODE env var" {
  CLAUDE_CODE=1 run lx init
  [ "$status" -eq 0 ]
  [[ "$output" == *"LYNX_CONTEXT=agent"* ]]
}
