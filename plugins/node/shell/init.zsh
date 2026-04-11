# node plugin — init.zsh
source "${LYNX_PLUGIN_DIR}/shell/functions.zsh"
add-zsh-hook chpwd node_gather_state
node_gather_state
