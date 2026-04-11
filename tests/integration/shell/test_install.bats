#!/usr/bin/env bats
# Integration tests for lx install command
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

@test "lx install --help exits 0 and shows usage" {
  run lx install --help
  [ "$status" -eq 0 ]
  [[ "$output" == *"install"* ]]
}

@test "lx install copies shell/core/hooks.zsh to LYNX_DIR" {
  local target="${HOME}/.config/lynx"
  run lx install --source "$LYNX_SOURCE_DIR" --dir "$target"
  [ "$status" -eq 0 ]
  [ -f "${target}/shell/core/hooks.zsh" ]
}

@test "lx install copies plugins/ to LYNX_DIR" {
  local target="${HOME}/.config/lynx"
  run lx install --source "$LYNX_SOURCE_DIR" --dir "$target"
  [ "$status" -eq 0 ]
  [ -d "${target}/plugins" ]
}

@test "lx install writes default config.toml" {
  local target="${HOME}/.config/lynx"
  run lx install --source "$LYNX_SOURCE_DIR" --dir "$target"
  [ "$status" -eq 0 ]
  [ -f "${target}/config.toml" ]
}

@test "lx install --zshrc patches ~/.zshrc" {
  local target="${HOME}/.config/lynx"
  run lx install --source "$LYNX_SOURCE_DIR" --dir "$target" --zshrc
  [ "$status" -eq 0 ]
  grep -q 'eval "$(lx init)"' "${HOME}/.zshrc"
}

@test "lx install --zshrc is idempotent" {
  local target="${HOME}/.config/lynx"
  lx install --source "$LYNX_SOURCE_DIR" --dir "$target" --zshrc
  lx install --source "$LYNX_SOURCE_DIR" --dir "$target" --zshrc
  local count
  count=$(grep -c 'eval "$(lx init)"' "${HOME}/.zshrc")
  [ "$count" -eq 1 ]
}

@test "installed hooks.zsh passes zsh syntax check" {
  local target="${HOME}/.config/lynx"
  lx install --source "$LYNX_SOURCE_DIR" --dir "$target"
  zsh -n "${target}/shell/core/hooks.zsh"
}

@test "lx prompt render produces PROMPT= after install" {
  local target="${HOME}/.config/lynx"
  lx install --source "$LYNX_SOURCE_DIR" --dir "$target"

  # Simulate what precmd does: set env and call lx prompt render
  export LYNX_DIR="$target"
  export LYNX_CONTEXT=interactive
  unset LYNX_CACHE_GIT_STATE

  run lx prompt render
  [ "$status" -eq 0 ]
  [[ "$output" == *"PROMPT="* ]]
}
