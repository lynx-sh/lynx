# golang plugin — init.zsh
source "${LYNX_PLUGIN_DIR}/shell/functions.zsh"
add-zsh-hook chpwd golang_gather_state
golang_gather_state
