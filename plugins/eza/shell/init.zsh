# eza plugin — init.zsh
# Sourced by eval-bridge via lx plugin exec eza.
# Aliases only — loaded in interactive context, never in agent/minimal.

[[ "$LYNX_CONTEXT" != "agent" && "$LYNX_CONTEXT" != "minimal" ]] && \
  source "${0:A:h}/aliases.zsh"
