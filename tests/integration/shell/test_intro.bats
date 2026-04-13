#!/usr/bin/env bats
# Integration tests for lx intro

load helpers

setup() {
  export HOME="$(mktemp -d)"
  export LYNX_TEST=1
  # Unset LYNX_DIR so HOME-based path resolution is used (not the dev machine's real config dir).
  unset LYNX_DIR
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

@test "lx intro list outputs all 5 built-in intros" {
  run lx intro list
  [ "$status" -eq 0 ]
  [[ "$output" == *"hacker"* ]]
  [[ "$output" == *"minimal"* ]]
  [[ "$output" == *"neofetch"* ]]
  [[ "$output" == *"welcome"* ]]
  [[ "$output" == *"poweruser"* ]]
}

@test "lx intro on enables intro in config" {
  run lx intro on
  [ "$status" -eq 0 ]
  local intro_section
  intro_section=$(awk '/^\[intro\]/{found=1} found && /^\[/{if(!/^\[intro\]/)found=0} found{print}' "$HOME/.config/lynx/config.toml")
  [[ "$intro_section" == *"enabled = true"* ]]
}

@test "lx intro off disables intro in config" {
  lx intro on
  run lx intro off
  [ "$status" -eq 0 ]
  local intro_section
  intro_section=$(awk '/^\[intro\]/{found=1} found && /^\[/{if(!/^\[intro\]/)found=0} found{print}' "$HOME/.config/lynx/config.toml")
  # [intro] enabled should not be true after off
  [[ "$intro_section" != *"enabled = true"* ]]
}

@test "lx intro set with valid slug succeeds and writes active in config" {
  run lx intro set minimal
  [ "$status" -eq 0 ]
  local config
  config=$(cat "$HOME/.config/lynx/config.toml")
  [[ "$config" == *"active = \"minimal\""* ]]
}

@test "lx intro set with nonexistent slug fails with non-zero exit" {
  run lx intro set this_slug_does_not_exist
  [ "$status" -ne 0 ]
}

@test "lx intro preview minimal prints non-empty output" {
  lx intro set minimal
  run lx intro preview minimal
  [ "$status" -eq 0 ]
  [ -n "$output" ]
}

@test "lx intro logo TEST prints multi-line ASCII art" {
  run lx intro logo "TEST"
  [ "$status" -eq 0 ]
  # Output should contain multiple lines
  local line_count
  line_count=$(echo "$output" | wc -l)
  [ "$line_count" -gt 1 ]
}

@test "lx intro logo with --list-fonts shows available fonts" {
  run lx intro logo "x" --list-fonts
  [ "$status" -eq 0 ]
  [[ "$output" == *"slant"* ]]
  [[ "$output" == *"standard"* ]]
}

@test "lx intro list marks active intro with asterisk" {
  lx intro set welcome
  run lx intro list
  [ "$status" -eq 0 ]
  [[ "$output" == *"* welcome"* ]]
}

@test "lx intro delete fails on built-in intro" {
  run lx intro delete hacker
  [ "$status" -ne 0 ]
  [[ "$output" == *"built-in"* ]] || [[ "$stderr" == *"built-in"* ]]
}
