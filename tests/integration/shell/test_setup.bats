#!/usr/bin/env bats
# Integration tests for lx setup command
# Each test runs in an isolated temp directory

setup() {
  export HOME="$(mktemp -d)"
  export LYNX_TEST=1
  # Determine the workspace root (where shell/ and plugins/ live)
  SCRIPT_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")" && pwd)"
  export LYNX_SOURCE_DIR="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
}

teardown() {
  rm -rf "$HOME"
}

@test "lx setup --help exits 0 and shows usage" {
  run lx setup --help
  [ "$status" -eq 0 ]
  [[ "$output" == *"install"* ]]
}

@test "lx setup copies shell/core/hooks.zsh to LYNX_DIR" {
  local target="${HOME}/.config/lynx"
  run lx setup --source "$LYNX_SOURCE_DIR" --dir "$target"
  [ "$status" -eq 0 ]
  [ -f "${target}/shell/core/hooks.zsh" ]
}

@test "lx setup copies plugins/ to LYNX_DIR" {
  local target="${HOME}/.config/lynx"
  run lx setup --source "$LYNX_SOURCE_DIR" --dir "$target"
  [ "$status" -eq 0 ]
  [ -d "${target}/plugins" ]
}

@test "lx setup writes default config.toml" {
  local target="${HOME}/.config/lynx"
  run lx setup --source "$LYNX_SOURCE_DIR" --dir "$target"
  [ "$status" -eq 0 ]
  [ -f "${target}/config.toml" ]
}

@test "lx setup --zshrc patches ~/.zshrc" {
  local target="${HOME}/.config/lynx"
  run lx setup --source "$LYNX_SOURCE_DIR" --dir "$target" --zshrc
  [ "$status" -eq 0 ]
  grep -q 'source "${HOME}/.config/lynx/shell/init.zsh"' "${HOME}/.zshrc"
}

@test "lx setup --zshrc is idempotent" {
  local target="${HOME}/.config/lynx"
  lx setup --source "$LYNX_SOURCE_DIR" --dir "$target" --zshrc
  lx setup --source "$LYNX_SOURCE_DIR" --dir "$target" --zshrc
  local count
  count=$(grep -c 'source "${HOME}/.config/lynx/shell/init.zsh"' "${HOME}/.zshrc")
  [ "$count" -eq 1 ]
}

@test "installed hooks.zsh passes zsh syntax check" {
  local target="${HOME}/.config/lynx"
  lx setup --source "$LYNX_SOURCE_DIR" --dir "$target"
  zsh -n "${target}/shell/core/hooks.zsh"
}

@test "lx prompt render produces PROMPT= after install" {
  local target="${HOME}/.config/lynx"
  lx setup --source "$LYNX_SOURCE_DIR" --dir "$target"

  # Simulate what precmd does: set env and call lx prompt render
  export LYNX_DIR="$target"
  export LYNX_CONTEXT=interactive
  unset LYNX_CACHE_GIT_STATE

  run lx prompt render
  [ "$status" -eq 0 ]
  [[ "$output" == *"PROMPT="* ]]
}
