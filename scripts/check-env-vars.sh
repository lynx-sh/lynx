#!/usr/bin/env bash
# check-env-vars.sh — deny-list guard for LYNX_* env-var literal drift
#
# Fails if any quoted "LYNX_*" string literal or direct std::env::var("LYNX_*")
# call appears outside the canonical source files listed in the allowlist below.
#
# Run locally:   bash scripts/check-env-vars.sh
# In CI:         called by scripts/verify-guardrails.sh (section 8)
#
# Allowlist:
#   crates/lynx-core/src/env_vars.rs   — canonical definitions + unit tests for
#                                        helper functions (plugin_guard_var, etc.)
#
# All other files must reference env-var names via the constants in env_vars.rs.
# Never add source files to this allowlist — fix the drift instead.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VIOLATIONS=0

# ── Patterns ─────────────────────────────────────────────────────────────────

# 1. Quoted LYNX_* string literals in Rust source
#    Matches: "LYNX_CONTEXT", "LYNX_CACHE_GIT_STATE", etc.
LITERAL_PATTERN='"LYNX_[A-Z0-9_]+"'

# 2. Direct std::env reads with a LYNX_* literal argument
#    Matches: std::env::var("LYNX_...), std::env::var_os("LYNX_...),
#             env::var("LYNX_...), env::var_os("LYNX_...)
ENV_READ_PATTERN='env::var(_os)?\("LYNX_'

# ── Allowlist ─────────────────────────────────────────────────────────────────

# Files whose path contains any of these substrings are skipped.
ALLOWLIST=(
    "crates/lynx-core/src/env_vars.rs"
)

is_allowed() {
    local file="$1"
    for allowed in "${ALLOWLIST[@]}"; do
        if [[ "$file" == *"$allowed"* ]]; then
            return 0
        fi
    done
    return 1
}

# ── Check function ────────────────────────────────────────────────────────────

check_pattern() {
    local label="$1"
    local pattern="$2"
    local results

    # Collect matches outside allowlist; rg exits 1 if no matches (that's fine)
    results=$(rg --with-filename --line-number "$pattern" \
        "$REPO_ROOT/crates" --type rust 2>/dev/null || true)

    if [[ -z "$results" ]]; then
        echo "  ✓ $label — 0 violations"
        return
    fi

    # Filter out allowlisted files
    local filtered=""
    while IFS= read -r line; do
        local file="${line%%:*}"
        if ! is_allowed "$file"; then
            filtered+="$line"$'\n'
        fi
    done <<< "$results"

    if [[ -z "$filtered" ]]; then
        echo "  ✓ $label — 0 violations (allowlist filtered)"
    else
        echo "  ✗ $label — found literal drift:"
        while IFS= read -r line; do
            [[ -z "$line" ]] && continue
            echo "      $line"
        done <<< "$filtered"
        echo "    Fix: replace with the constant from crates/lynx-core/src/env_vars.rs"
        VIOLATIONS=$((VIOLATIONS + 1))
    fi
}

# ── Run checks ────────────────────────────────────────────────────────────────

echo "── Env-var literal drift ────────────────────────────────────────────────────"
check_pattern 'LYNX_* quoted string literals outside env_vars.rs' "$LITERAL_PATTERN"
check_pattern 'Direct env::var("LYNX_*") calls outside env_vars.rs' "$ENV_READ_PATTERN"

echo

if [[ "$VIOLATIONS" -gt 0 ]]; then
    echo "  FAIL: ${VIOLATIONS} env-var drift violation(s) — see above"
    exit 1
fi

echo "  All env-var checks passed."
exit 0
