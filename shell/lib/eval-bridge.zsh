# Lynx eval bridge — safe patterns for evaling lx output.
# Never eval unchecked output. Always verify exit code first.

# Usage: lynx_eval_plugin <plugin-name>
# Evals the shell activation output for a named plugin.
lynx_eval_plugin() {
  local plugin="$1"
  local output exit_code
  output="$(lx plugin exec "$plugin" 2>&1)"
  exit_code=$?
  if (( exit_code != 0 )); then
    print -u2 "Lynx: plugin '${plugin}' failed (exit ${exit_code}). Run: lx doctor"
    return 1
  fi
  eval "$output"
}

# Usage: lynx_eval_safe <lx subcommand args...>
# General-purpose safe wrapper: evals any lx subcommand output only on success.
lynx_eval_safe() {
  local output exit_code
  output="$(lx "$@" 2>&1)"
  exit_code=$?
  if (( exit_code != 0 )); then
    print -u2 "Lynx: 'lx $*' failed (exit ${exit_code}). Run: lx doctor"
    return 1
  fi
  eval "$output"
}
