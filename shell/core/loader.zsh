# Lynx loader: thin eval-bridge entrypoint only.
# Unset inherited state from parent shells — each new shell must initialize fresh.
unset LYNX_INITIALIZED
eval "$(lx init 2>/dev/null)"
