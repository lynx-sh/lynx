# Lynx hook bridge — translates zsh hooks into lx events.
# Pure forwarders only. No business logic. Failures are always silent.

autoload -Uz add-zsh-hook

_lynx_hook_chpwd() {
  lx event emit "shell:chpwd" --data "$PWD" 2>/dev/null
}

_lynx_hook_preexec() {
  lx event emit "shell:preexec" --data "$1" 2>/dev/null
}

_lynx_hook_precmd() {
  # Marshal zsh-side state caches into env vars before calling lx prompt render.
  export LYNX_CACHE_GIT_STATE="{\"branch\":\"${_lynx_git_state[branch]:-}\",\"dirty\":\"${_lynx_git_state[dirty]:-0}\",\"stash\":\"${_lynx_git_state[stash]:-0}\",\"ahead\":\"${_lynx_git_state[ahead]:-0}\",\"behind\":\"${_lynx_git_state[behind]:-0}\"}"
  export LYNX_CACHE_KUBECTL_STATE="{\"context\":\"${_lynx_kubectl_state[context]:-}\",\"namespace\":\"${_lynx_kubectl_state[namespace]:-default}\"}"
  eval "$(lx prompt render 2>/dev/null)"
  lx event emit "shell:precmd" 2>/dev/null
}

add-zsh-hook chpwd   _lynx_hook_chpwd
add-zsh-hook preexec _lynx_hook_preexec
add-zsh-hook precmd  _lynx_hook_precmd
