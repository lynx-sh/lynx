# Lynx hook bridge — translates zsh lifecycle events into lx calls.
# Pure forwarders only. No business logic. Failures are always silent.

autoload -Uz add-zsh-hook

_lynx_hook_chpwd() {
  lx event emit "shell:chpwd" --data "$PWD" 2>/dev/null
}

_lynx_hook_preexec() {
  lx event emit "shell:preexec" --data "$1" 2>/dev/null
}

_lynx_hook_precmd() {
  export LYNX_LAST_EXIT_CODE=$?
  export LYNX_BG_JOBS=${#jobstates}
  eval "$(lx refresh-state 2>/dev/null)"
  eval "$(lx prompt render 2>/dev/null)"
  lx event emit "shell:precmd" 2>/dev/null
}

# Transient prompt: collapse the full prompt to a minimal one after each command.
# Activated only when lx prompt render --transient is available (zle widget).
_lynx_zle_line_finish() {
  eval "$(lx prompt render --transient 2>/dev/null)"
  zle reset-prompt 2>/dev/null
}
zle -N _lynx_zle_line_finish

add-zsh-hook chpwd   _lynx_hook_chpwd
add-zsh-hook preexec _lynx_hook_preexec
add-zsh-hook precmd  _lynx_hook_precmd
