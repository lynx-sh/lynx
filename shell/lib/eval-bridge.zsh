# Lynx eval bridge: no business logic, only eval forwarding.
lynx_eval_plugin() { eval "$(lx plugin exec "$1" 2>/dev/null)"; }
lynx_eval_safe() { eval "$(lx "$@" 2>/dev/null)"; }
