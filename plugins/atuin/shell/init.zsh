# atuin plugin — init.zsh
# Wires atuin's zsh integration via the eval-bridge pattern.
# Atuin handles its own CTRL-R binding — do not set ATUIN_NOBIND=true.
if ! command -v atuin &>/dev/null; then
  echo "lynx: atuin plugin requires atuin — install with: brew install atuin" >&2
  return 1
fi

# Sensible defaults — set before eval so atuin init sees them
export ATUIN_FILTER_MODE="${ATUIN_FILTER_MODE:-host}"
export ATUIN_STYLE="${ATUIN_STYLE:-auto}"

eval "$(atuin init zsh 2>/dev/null)"
