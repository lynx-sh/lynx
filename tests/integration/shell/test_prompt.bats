#!/usr/bin/env bats
# Integration tests for lx prompt render

setup() {
  export HOME="$(mktemp -d)"
  export LYNX_TEST=1
  export PWD="$HOME"
  export LYNX_CONTEXT=interactive
  unset LYNX_CACHE_GIT_STATE LYNX_CACHE_KUBECTL_STATE LYNX_LAST_CMD_MS

  # Provide themes so `lx prompt render` can load them without a full `lx setup`.
  # LYNX_DIR defaults to ~/.config/lynx; we point it at a temp dir with themes copied in.
  local repo_root
  repo_root="$(cd "$BATS_TEST_DIRNAME/../../.." && pwd)"
  export LYNX_DIR="$HOME/.config/lynx"
  mkdir -p "$LYNX_DIR/themes"
  cp "$repo_root"/themes/*.toml "$LYNX_DIR/themes/"
}

teardown() {
  rm -rf "$HOME"
}

@test "lx prompt render exits successfully" {
  run lx prompt render
  [ "$status" -eq 0 ]
}

@test "lx prompt render outputs PROMPT assignment" {
  run lx prompt render
  [ "$status" -eq 0 ]
  [[ "$output" == *"PROMPT="* ]]
}

@test "lx prompt render outputs RPROMPT assignment" {
  run lx prompt render
  [ "$status" -eq 0 ]
  [[ "$output" == *"RPROMPT="* ]]
}

@test "lx prompt render output is valid zsh" {
  run lx prompt render
  [ "$status" -eq 0 ]
  echo "$output" | zsh -n
}

@test "lx prompt render with git state env var includes branch in output" {
  export LYNX_CACHE_GIT_STATE='{"branch":"feature/test","dirty":"0","stash":"0","ahead":"0","behind":"0"}'
  run lx prompt render
  [ "$status" -eq 0 ]
  [[ "$output" == *"feature/test"* ]]
}

@test "lx prompt render in agent context produces valid zsh" {
  export LYNX_CONTEXT=agent
  run lx prompt render
  [ "$status" -eq 0 ]
  echo "$output" | zsh -n
}

@test "lx prompt render with LYNX_THEME=minimal produces valid zsh" {
  export LYNX_THEME=minimal
  run lx prompt render
  [ "$status" -eq 0 ]
  echo "$output" | zsh -n
}
