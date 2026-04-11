# rust-ver plugin — init.zsh
source "${LYNX_PLUGIN_DIR}/shell/functions.zsh"
add-zsh-hook chpwd rust_ver_gather_state
rust_ver_gather_state
