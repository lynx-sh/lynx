# Lynx command dispatch — shell function wrapper for lx subcommands that
# emit eval-able output. Subcommands that produce shell assignments (export
# statements) must be wrapped with eval so they take effect in the current
# shell. All other invocations pass through directly to the lx binary.
lx() {
  case "$1 $2" in
    "theme set"|"context set") eval "$(command lx "$@" 2>/dev/null)" ;;
    *) command lx "$@" ;;
  esac
}
