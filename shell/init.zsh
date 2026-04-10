# Lynx shell integration — sourced from .zshrc
# This is the ONLY line needed in .zshrc:
#   source "${HOME}/.config/lynx/init.zsh"
#
# Everything else is managed by `lx`.

source "${LYNX_DIR:-${HOME}/.config/lynx}/shell/core/loader.zsh"
