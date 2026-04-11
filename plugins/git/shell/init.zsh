# git plugin — init.zsh
source "${LYNX_PLUGIN_DIR}/git/shell/functions.zsh"
[[ "$LYNX_CONTEXT" != "agent" && "$LYNX_CONTEXT" != "minimal" ]] && \
  source "${LYNX_PLUGIN_DIR}/git/shell/aliases.zsh"
git_refresh_state
