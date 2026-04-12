# Lynx hook bridge — translates zsh lifecycle events into lx calls.
# Pure forwarders only. No business logic. Failures are always silent.
autoload -Uz add-zsh-hook

_lynx_hook_chpwd() {
  lx event emit "shell:chpwd" --data "$PWD" 2>/dev/null
}

_lynx_hook_preexec() {
  lx event emit "shell:preexec" --data "$1" 2>/dev/null
}

# File descriptor used for async refresh-state communication.
_lynx_async_fd=

# Called by zle when refresh-state output is ready. Evals fresh state, re-renders.
_lynx_async_done() {
  local fd=$1
  local output
  output=$(cat <&$fd 2>/dev/null)
  zle -F $fd 2>/dev/null
  exec {_lynx_async_fd}<&- 2>/dev/null
  _lynx_async_fd=
  eval "$output" 2>/dev/null
  eval "$(COLUMNS=$COLUMNS lx prompt render 2>/dev/null)"
  zle && zle reset-prompt 2>/dev/null
}

_lynx_hook_precmd() {
  export LYNX_LAST_EXIT_CODE=$?
  export LYNX_BG_JOBS=${#jobstates}
  export LYNX_NOW_SECS=$(date +%s)

  # Cancel any in-flight async refresh from the previous cycle.
  if [[ -n ${_lynx_async_fd:-} ]]; then
    zle -F $_lynx_async_fd 2>/dev/null
    exec {_lynx_async_fd}<&- 2>/dev/null
    _lynx_async_fd=
  fi

  # Phase 1: render immediately with stale state from last cycle.
  eval "$(COLUMNS=$COLUMNS lx prompt render 2>/dev/null)"

  # Phase 2: gather fresh state async; re-render via _lynx_async_done.
  exec {_lynx_async_fd}< <(lx refresh-state 2>/dev/null)
  zle -F $_lynx_async_fd _lynx_async_done 2>/dev/null

  lx event emit "shell:precmd" 2>/dev/null
}

# Transient prompt: collapse the full prompt to a minimal one after each command.
_lynx_zle_line_finish() {
  eval "$(lx prompt render --transient 2>/dev/null)"
  zle reset-prompt 2>/dev/null
}
zle -N _lynx_zle_line_finish

add-zsh-hook chpwd   _lynx_hook_chpwd
add-zsh-hook preexec _lynx_hook_preexec
add-zsh-hook precmd  _lynx_hook_precmd
