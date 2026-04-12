# golang plugin — init.zsh
source "${0:A:h}/functions.zsh"
add-zsh-hook chpwd golang_gather_state
golang_gather_state
