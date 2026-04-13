# git plugin — init.zsh
source "${0:A:h}/functions.zsh"
[[ "$LYNX_CONTEXT" != "agent" && "$LYNX_CONTEXT" != "minimal" ]] && \
  source "${0:A:h}/aliases.zsh"
git_refresh_state
