# Lynx command dispatch — shell function wrapper for lx subcommands that
# emit eval-able output. Subcommands that produce shell assignments (export
# statements) must be wrapped with eval so they take effect in the current
# shell. All other invocations pass through directly to the lx binary.
lx() {
  case "$1 $2" in
    "context set") eval "$(command lx "$@")" ;;
    # alias add: binary emits "alias name='cmd'" + feedback line; eval sets alias live.
    "alias add")   eval "$(command lx "$@")" ;;
    # alias remove: binary removes from config; unalias clears the current session.
    "alias remove") command lx "$@" && unalias "$3" 2>/dev/null ;;
    *) command lx "$@" ;;
  esac
}
