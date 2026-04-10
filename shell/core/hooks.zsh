# Lynx hook bridge — translates zsh hooks into lx events.
# Pure forwarders only. No logic. Failures are always silent.
# Set LYNX_HOOK_DEBUG=1 to enable verbose output for debugging.

autoload -Uz add-zsh-hook

_lynx_hook_chpwd() {
  lx event emit "shell:chpwd" --data "$PWD" 2>/dev/null
  [[ -n "${LYNX_HOOK_DEBUG}" ]] && print -u2 "[lynx:hook] chpwd -> $PWD"
}

_lynx_hook_preexec() {
  lx event emit "shell:preexec" --data "$1" 2>/dev/null
  [[ -n "${LYNX_HOOK_DEBUG}" ]] && print -u2 "[lynx:hook] preexec -> $1"
}

_lynx_hook_precmd() {
  lx event emit "shell:precmd" 2>/dev/null
  [[ -n "${LYNX_HOOK_DEBUG}" ]] && print -u2 "[lynx:hook] precmd"
}

add-zsh-hook chpwd   _lynx_hook_chpwd
add-zsh-hook preexec _lynx_hook_preexec
add-zsh-hook precmd  _lynx_hook_precmd
