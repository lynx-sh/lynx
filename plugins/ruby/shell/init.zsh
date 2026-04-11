# ruby plugin — init.zsh
source "${LYNX_PLUGIN_DIR}/shell/functions.zsh"
add-zsh-hook chpwd ruby_gather_state
ruby_gather_state
