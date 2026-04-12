# ruby plugin — init.zsh
source "${0:A:h}/functions.zsh"
add-zsh-hook chpwd ruby_gather_state
ruby_gather_state
