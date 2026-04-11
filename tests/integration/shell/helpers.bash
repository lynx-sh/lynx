# Canonical env var names — mirrors crates/lynx-core/src/env_vars.rs.
# Source this file in every bats test file:
#   load helpers
#
# Never hardcode these strings in tests — use the constants below.

# Agent detection vars (matches env_vars::CLAUDECODE and env_vars::CURSOR_CLI)
LYNX_VAR_CLAUDECODE="CLAUDECODE"
LYNX_VAR_CURSOR_CLI="CURSOR_CLI"
LYNX_VAR_CI="CI"

# Lynx runtime vars (matches env_vars::LYNX_CONTEXT, etc.)
LYNX_VAR_CONTEXT="LYNX_CONTEXT"
LYNX_VAR_DIR="LYNX_DIR"
