# atuin plugin init: thin glue only.
export ATUIN_FILTER_MODE="${ATUIN_FILTER_MODE:-host}"
export ATUIN_STYLE="${ATUIN_STYLE:-auto}"
eval "$(atuin init zsh 2>/dev/null)"
