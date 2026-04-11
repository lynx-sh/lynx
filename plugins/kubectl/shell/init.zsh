# kubectl plugin — init.zsh
if ! command -v kubectl &>/dev/null; then
  echo "lynx: kubectl plugin requires kubectl — install from https://kubernetes.io/docs/tasks/tools/" >&2
  return 1
fi
source "${LYNX_PLUGIN_DIR}/kubectl/shell/functions.zsh"
kubectl_refresh_state
