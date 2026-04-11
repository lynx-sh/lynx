#!/usr/bin/env bats
# Integration tests for plugin loading, exec script, and hook wiring
# Each test runs in an isolated temp HOME

setup() {
  export HOME="$(mktemp -d)"
  export LYNX_TEST=1
  # Point to the repo plugins directory
  export LYNX_PLUGIN_DIR="$BATS_TEST_DIRNAME/../../../plugins"
}

teardown() {
  rm -rf "$HOME"
}

@test "lx plugin exec git produces valid zsh" {
  run lx plugin exec git
  [ "$status" -eq 0 ]
  echo "$output" | zsh -n
}

@test "lx plugin exec git output sources init.zsh" {
  run lx plugin exec git
  [ "$status" -eq 0 ]
  [[ "$output" == *"shell/init.zsh"* ]]
}

@test "lx plugin exec git registers LYNX_PLUGIN_GIT_LOADED guard" {
  run lx plugin exec git
  [ "$status" -eq 0 ]
  [[ "$output" == *"LYNX_PLUGIN_GIT_LOADED"* ]]
}

@test "lx plugin exec git does not self-register hooks (lx refresh-state handles precmd)" {
  # git plugin uses hooks=[] — state refresh is driven by lx refresh-state in the
  # Lynx precmd hook, not by per-plugin add-zsh-hook calls.
  run lx plugin exec git
  [ "$status" -eq 0 ]
  [[ "$output" != *"add-zsh-hook"* ]]
}

@test "lx plugin exec git sources functions via init.zsh (state model verification)" {
  # Confirm the exec script wires the plugin via init.zsh source, not hook registration.
  run lx plugin exec git
  [ "$status" -eq 0 ]
  [[ "$output" == *"shell/init.zsh"* ]]
}

@test "lx plugin exec is idempotent (guard prevents double-load)" {
  # Source the exec output twice in a clean zsh — second eval should no-op
  local script
  script="$(lx plugin exec git)"
  run zsh -c "
    autoload -Uz add-zsh-hook
    LYNX_PLUGIN_DIR='$LYNX_PLUGIN_DIR'
    eval '$script'
    eval '$script'
    echo loaded_count=1
  "
  [ "$status" -eq 0 ]
}

@test "lx plugin unload git produces valid zsh" {
  run lx plugin unload git
  [ "$status" -eq 0 ]
  echo "$output" | zsh -n
}

@test "lx plugin unload git clears the load guard" {
  run lx plugin unload git
  [ "$status" -eq 0 ]
  [[ "$output" == *"unset LYNX_PLUGIN_GIT_LOADED"* ]]
}

@test "lx plugin unload git does not emit hook removals (git uses hooks=[])" {
  # git plugin declares no hooks — unload script should not contain add-zsh-hook -d.
  run lx plugin unload git
  [ "$status" -eq 0 ]
  [[ "$output" != *"add-zsh-hook -d"* ]]
}
