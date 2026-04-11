# git plugin — functions.zsh
# All git state is cached into _lynx_git_state[] on chpwd/precmd.
# Prompt segments read from the cache — no git calls in render path.

typeset -gA _lynx_git_state

# Refresh the git state cache. Called on chpwd and precmd hooks.
# All logic lives in Rust (lx git-state) — D-001.
git_refresh_state() { eval "$(lx git-state 2>/dev/null)" }

# Public helpers — read from cache (fast, safe in prompt)
git_branch()      { echo "${_lynx_git_state[branch]}" }
git_dirty()       { echo "${_lynx_git_state[dirty]:-0}" }
git_staged()      { echo "${_lynx_git_state[staged]:-0}" }
git_modified()    { echo "${_lynx_git_state[modified]:-0}" }
git_untracked()   { echo "${_lynx_git_state[untracked]:-0}" }
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
