# Standard pattern for plugins to eval lx output.
# Usage: lynx_eval_plugin <plugin-name>
lynx_eval_plugin() {
  local plugin="$1"
  local output
  output="$(lx plugin exec "$plugin" 2>&1)" || {
    print -u2 "Lynx: plugin '$plugin' failed to exec. Run: lx doctor"
    return 1
  }
  eval "$output"
}
