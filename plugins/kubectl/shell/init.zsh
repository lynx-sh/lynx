# kubectl plugin — init.zsh
# Binary guard and context gate are enforced by lx plugin exec (Rust). No logic here.
source "${LYNX_PLUGIN_DIR}/kubectl/shell/functions.zsh"
kubectl_refresh_state
