# autosuggestions plugin — init.zsh
# Vendors zsh-autosuggestions with theme-integrated colors from Lynx.
# ZSH_AUTOSUGGEST_HIGHLIGHT_STYLE is pre-set by lx init from the active theme.

source "${0:A:h}/../vendor/zsh-autosuggestions.zsh"

# Smart Tab widget — accept the visible autosuggestion if one is showing,
# otherwise fall through to normal zsh completion (subcommands, files, etc.).
# This makes Tab behave intuitively on laptops without an End key.
_lynx_tab_or_complete() {
  if [[ -n "$POSTDISPLAY" ]]; then
    # A suggestion is visible — accept it.
    zle autosuggest-accept
  else
    # No suggestion — run normal completion.
    zle complete-word
  fi
}
zle -N _lynx_tab_or_complete
bindkey '^I' _lynx_tab_or_complete
