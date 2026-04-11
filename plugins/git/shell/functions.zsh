# git plugin — functions.zsh
# All git state is cached into _lynx_git_state[] on chpwd/precmd.
# Prompt segments read from the cache — no git calls in render path.

typeset -gA _lynx_git_state

# Refresh the git state cache. Called on chpwd and precmd hooks.
git_refresh_state() {
  local root branch dirty stash ahead behind

  root=$(git rev-parse --show-toplevel 2>/dev/null)
  if [[ -z "$root" ]]; then
    _lynx_git_state=()
    return 0
  fi

  branch=$(git symbolic-ref --short HEAD 2>/dev/null || git rev-parse --short HEAD 2>/dev/null)
  dirty=$([[ -n "$(git status --porcelain 2>/dev/null)" ]] && echo "1" || echo "0")
  stash=$(git stash list 2>/dev/null | wc -l | tr -d ' ')

  local upstream
  upstream=$(git rev-parse --abbrev-ref --symbolic-full-name @{u} 2>/dev/null)
  if [[ -n "$upstream" ]]; then
    local counts
    counts=$(git rev-list --left-right --count HEAD..."$upstream" 2>/dev/null)
    ahead=${counts%$'\t'*}
    behind=${counts#*$'\t'}
  else
    ahead=0
    behind=0
  fi

  _lynx_git_state=(
    root    "$root"
    branch  "$branch"
    dirty   "$dirty"
    stash   "$stash"
    ahead   "$ahead"
    behind  "$behind"
  )
}

# Public helpers — read from cache (fast, safe in prompt)
git_branch()      { echo "${_lynx_git_state[branch]}" }
git_dirty()       { echo "${_lynx_git_state[dirty]:-0}" }
git_stash_count() { echo "${_lynx_git_state[stash]:-0}" }
git_root()        { echo "${_lynx_git_state[root]}" }

# Returns "+A/-B" string or empty if no upstream
git_ahead_behind() {
  local a="${_lynx_git_state[ahead]:-0}" b="${_lynx_git_state[behind]:-0}"
  [[ "$a" == "0" && "$b" == "0" ]] && return 0
  echo "+${a}/-${b}"
}

# Hook targets — registered via plugin.toml hooks[]
_git_plugin_chpwd()  { git_refresh_state }
_git_plugin_precmd() { git_refresh_state }
