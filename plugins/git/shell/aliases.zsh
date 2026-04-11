# git plugin — aliases.zsh
# Loaded only in interactive context (plugin.toml: disabled_in = ["agent","minimal"])
# User-defined aliases always win — we only set an alias if not already defined.

_lynx_alias() { (( ${+aliases[$1]} )) || alias "$1=$2"; }

_lynx_alias gst  'git status'
_lynx_alias gco  'git checkout'
_lynx_alias gcm  'git commit -m'
_lynx_alias glog 'git log --oneline --graph --decorate'
_lynx_alias gd   'git diff'
_lynx_alias ga   'git add'
_lynx_alias gp   'git push'
_lynx_alias gl   'git pull'
_lynx_alias gsh  'git stash'
_lynx_alias grb  'git rebase'
_lynx_alias gsw  'git switch'

unfunction _lynx_alias
