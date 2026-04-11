# fzf plugin — keybinds.zsh
# Keybindings registered only in interactive context.
# CTRL-R — fzf history search (replaces zsh default reverse-history-search)
# CTRL-T — fzf file search from cwd, selected path appended to command line
# ALT-C  — fzf directory jump with preview

# Skip if these widgets are already bound (user has own fzf setup)
_fzf_bind_if_unset() {
  local key="$1" widget="$2"
  local current
  current=$(bindkey "$key" 2>/dev/null | awk '{print $2}')
  [[ "$current" == "$widget" ]] && return 0  # already ours — idempotent
  bindkey "$key" "$widget"
}

# CTRL-R: fzf history widget
_fzf_history_widget() {
  local selected
  selected=$(fc -rl 1 \
    | awk '!seen[$0]++' \
    | fzf --no-sort --reverse --tiebreak=index \
        --prompt="history> " --height=40% \
        --query="${LBUFFER}" \
        --preview-window=hidden)
  if [[ -n "$selected" ]]; then
    LBUFFER="${selected#* }"
    zle redisplay
  fi
}
zle -N _fzf_history_widget
_fzf_bind_if_unset "^R" _fzf_history_widget

# CTRL-T: fzf file search
_fzf_file_widget() {
  local selected
  selected=$(find . -not -path '*/\.*' 2>/dev/null \
    | fzf --prompt="file> " --height=40% --preview='ls -la {}')
  [[ -n "$selected" ]] && LBUFFER+="$selected"
  zle redisplay
}
zle -N _fzf_file_widget
_fzf_bind_if_unset "^T" _fzf_file_widget

# ALT-C: fzf directory jump
_fzf_cd_widget() {
  local dir
  dir=$(find . -type d -not -path '*/\.*' 2>/dev/null \
    | fzf --prompt="cd> " --height=40% --preview='ls {}')
  if [[ -n "$dir" ]]; then
    cd "$dir" || return 1
    zle reset-prompt
  fi
}
zle -N _fzf_cd_widget
_fzf_bind_if_unset "^[c" _fzf_cd_widget
