# fzf plugin — init.zsh
if ! command -v fzf &>/dev/null; then
  echo "lynx: fzf plugin requires fzf — install with: brew install fzf" >&2
  return 1
fi
source "${LYNX_PLUGIN_DIR}/fzf/shell/functions.zsh"
[[ "$LYNX_CONTEXT" != "agent" && "$LYNX_CONTEXT" != "minimal" ]] && \
  source "${LYNX_PLUGIN_DIR}/fzf/shell/keybinds.zsh"
