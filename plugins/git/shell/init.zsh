# git plugin — init.zsh
source "${LYNX_PLUGIN_DIR}/shell/functions.zsh"
[[ "$LYNX_CONTEXT" != "agent" && "$LYNX_CONTEXT" != "minimal" ]] && \
  source "${LYNX_PLUGIN_DIR}/shell/aliases.zsh"
git_refresh_state
