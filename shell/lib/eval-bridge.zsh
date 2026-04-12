# Lynx eval bridge: no business logic, only eval forwarding.
# Errors go to the diagnostic log (lx diag) rather than being silently dropped.
lynx_eval_plugin() { eval "$(lx plugin exec "$1" 2>>"${LYNX_DIR}/logs/lx-diag.log")"; }
lynx_eval_safe() { eval "$(lx "$@" 2>>"${LYNX_DIR}/logs/lx-diag.log")"; }
