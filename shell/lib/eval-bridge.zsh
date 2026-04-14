# Lynx eval bridge: no business logic, only eval forwarding.
# Errors go to the diagnostic log (lx diag) rather than being silently dropped.
#
# D-001 exception: the stderr redirect and exit-code check below are I/O
# plumbing, not logic. They exist solely to route zsh eval errors (e.g.
# "(eval):N: unmatched '") through `lx shell-error` so users see a styled
# LynxError::Shell instead of a raw zsh message. All formatting stays in Rust.
lynx_eval_plugin() {
  local _lynx_out _lynx_err _lynx_tmp
  _lynx_tmp=$(mktemp)
  _lynx_out=$(lx plugin exec "$1" 2>>"${LYNX_DIR}/logs/lx-diag.log")
  eval "$_lynx_out" 2>"$_lynx_tmp"
  if (( $? != 0 )); then
    _lynx_err=$(<"$_lynx_tmp")
    [[ -n "$_lynx_err" ]] && lx shell-error "$_lynx_err" >&2
  fi
  command rm -f "$_lynx_tmp"
}
lynx_eval_safe() {
  local _lynx_out _lynx_err _lynx_tmp
  _lynx_tmp=$(mktemp)
  _lynx_out=$(lx "$@" 2>>"${LYNX_DIR}/logs/lx-diag.log")
  eval "$_lynx_out" 2>"$_lynx_tmp"
  if (( $? != 0 )); then
    _lynx_err=$(<"$_lynx_tmp")
    [[ -n "$_lynx_err" ]] && lx shell-error "$_lynx_err" >&2
  fi
  command rm -f "$_lynx_tmp"
}
