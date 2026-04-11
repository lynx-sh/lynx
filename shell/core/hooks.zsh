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
  eval "$(lx refresh-state 2>/dev/null)"
  eval "$(lx prompt render 2>/dev/null)"
  lx event emit "shell:precmd" 2>/dev/null
}

add-zsh-hook chpwd   _lynx_hook_chpwd
add-zsh-hook preexec _lynx_hook_preexec
add-zsh-hook precmd  _lynx_hook_precmd
