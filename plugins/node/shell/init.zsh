# node plugin — init.zsh
source "${0:A:h}/functions.zsh"
add-zsh-hook chpwd node_gather_state
node_gather_state
