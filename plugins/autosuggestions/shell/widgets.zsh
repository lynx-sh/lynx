# autosuggestions plugin — widgets.zsh
# Smart Tab widget: accept autosuggestion if visible, else fall through to completion.

_lynx_tab_or_complete() {
  if [[ -n "$POSTDISPLAY" ]]; then
    zle autosuggest-accept
  else
    zle complete-word
  fi
}
zle -N _lynx_tab_or_complete
bindkey '^I' _lynx_tab_or_complete
