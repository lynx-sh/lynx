# rust-ver plugin — init.zsh
source "${0:A:h}/functions.zsh"
add-zsh-hook chpwd rust_ver_gather_state
rust_ver_gather_state
