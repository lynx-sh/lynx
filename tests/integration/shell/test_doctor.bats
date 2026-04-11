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
