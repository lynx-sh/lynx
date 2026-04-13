#!/usr/bin/env bash
# verify-guardrails.sh — unified offline guardrail checker for Lynx
#
# Runs all invariant checks that protect against known architecture drift:
#   1. Shell protocol violations (line limits, branching)
#   2. Context mismatch (env var detector constants)
#   3. Dependency map drift (forbidden crate deps)
#   4. Index/lock checksum enforcement (validate_index in pipeline)
#   5. Docs-command mismatch (critical commands in README)
#
# Exits 0 only when all checks pass.
# Usage: scripts/verify-guardrails.sh [--bats-only | --cargo-only]

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PASS=0
FAIL=0
FAILURES=()
# Use += instead of ((var++)) to avoid set -e triggering on arithmetic 0→1 edge

_ok()   { echo "  ✓ $*"; }
_fail() { echo "  ✗ $*"; FAILURES+=("$*"); FAIL=$((FAIL + 1)); }
_hdr()  { echo; echo "── $* ──"; }

# ── 1. Shell protocol violations ─────────────────────────────────────────────

_hdr "Shell protocol violations"

check_line_limit() {
  local file="$1" limit="$2"
  if [ ! -f "$file" ]; then _fail "missing: $file"; return; fi
  local count
  count=$(wc -l < "$file" | tr -d ' ')
  if [ "$count" -le "$limit" ]; then
    _ok "$(basename "$file") — ${count}/${limit} lines"
    PASS=$((PASS + 1))
  else
    _fail "$(basename "$file") — ${count} lines exceeds ${limit}-line limit"
  fi
}

check_line_limit "$REPO_ROOT/shell/core/loader.zsh" 60
check_line_limit "$REPO_ROOT/shell/core/hooks.zsh" 60
check_line_limit "$REPO_ROOT/shell/lib/eval-bridge.zsh" 60

while IFS= read -r -d '' init_file; do
  check_line_limit "$init_file" 10
done < <(find "$REPO_ROOT/plugins" -name "init.zsh" -print0)

# Branching keywords must not appear in thin shell files
BRANCH_PATTERN='(^|[[:space:]])(if|for|while|case)([[:space:]]|$)'
THIN_FILES=(
  "$REPO_ROOT/shell/core/loader.zsh"
  "$REPO_ROOT/shell/lib/eval-bridge.zsh"
)
while IFS= read -r -d '' f; do THIN_FILES+=("$f"); done \
  < <(find "$REPO_ROOT/plugins" -name "init.zsh" -print0)

if rg -q "$BRANCH_PATTERN" "${THIN_FILES[@]}" 2>/dev/null; then
  _fail "branching keywords found in thin shell file(s)"
else
  _ok "no branching keywords in thin shell files"
  PASS=$((PASS + 1))
fi

# ── 2. Context mismatch ───────────────────────────────────────────────────────

_hdr "Context mismatch"

check_grep() {
  local label="$1" pattern="$2"; shift 2
  if rg -q "$pattern" "$@" 2>/dev/null; then
    _ok "$label"
    PASS=$((PASS + 1))
  else
    _fail "$label — pattern not found: $pattern"
  fi
}

CONTEXT_SRC="$REPO_ROOT/crates/lynx-shell/src/context.rs"
check_grep "CLAUDECODE in detector source" "CLAUDECODE" "$CONTEXT_SRC"
check_grep "CURSOR_CLI in detector source" "CURSOR_CLI" "$CONTEXT_SRC"
check_grep "CI → minimal mapping in source" "MINIMAL_ENV_VARS" "$CONTEXT_SRC"
check_grep "context value 'interactive' in types" "Interactive" "$REPO_ROOT/crates/lynx-core/src/types.rs"
check_grep "context value 'agent' in types" "Agent" "$REPO_ROOT/crates/lynx-core/src/types.rs"
check_grep "context value 'minimal' in types" "Minimal" "$REPO_ROOT/crates/lynx-core/src/types.rs"

# ── 3. Dependency map drift ───────────────────────────────────────────────────

_hdr "Dependency map drift"

check_no_dep() {
  local crate="$1" forbidden="$2"
  local cargo="$REPO_ROOT/crates/$crate/Cargo.toml"
  if [ ! -f "$cargo" ]; then _fail "missing Cargo.toml: $crate"; return; fi
  # Match only dependency declarations (leading whitespace + crate name), not comments or crate name= lines
  if rg -q "^\s*${forbidden}\s*=" "$cargo" 2>/dev/null; then
    _fail "$crate must not depend on $forbidden"
  else
    _ok "$crate has no $forbidden dep"
    PASS=$((PASS + 1))
  fi
}

# lynx-core: zero internal lynx-* deps (match only dependency declarations, not crate name line)
if rg -q '^\s*lynx-\w' "$REPO_ROOT/crates/lynx-core/Cargo.toml" 2>/dev/null; then
  _fail "lynx-core has internal lynx-* dependency (D-001 violation)"
else
  _ok "lynx-core: no internal lynx-* deps"
  PASS=$((PASS + 1))
fi

check_no_dep "lynx-prompt" "lynx-loader"   # circular
check_no_dep "lynx-events" "lynx-plugin"   # circular
check_no_dep "lynx-shell"  "lynx-cli"

# No crate except lynx-cli may depend on lynx-cli
bad_cli_deps=$(find "$REPO_ROOT/crates" -name "Cargo.toml" \
  ! -path "*/lynx-cli/*" \
  -exec rg -l '^\s*lynx-cli\s*=' {} + 2>/dev/null || true)
if [ -n "$bad_cli_deps" ]; then
  _fail "crate(s) depend on lynx-cli (not allowed): $bad_cli_deps"
else
  _ok "no non-cli crate depends on lynx-cli"
  PASS=$((PASS + 1))
fi

# ── 4. Index/lock checksum enforcement ───────────────────────────────────────

_hdr "Index/lock checksum enforcement"

INDEX_SRC="$REPO_ROOT/crates/lynx-registry/src/index.rs"
FETCH_SRC="$REPO_ROOT/crates/lynx-registry/src/fetch.rs"

if rg -q "fn validate_index" "$INDEX_SRC" 2>/dev/null; then
  _ok "validate_index fn exists in lynx-registry/index.rs"
  PASS=$((PASS + 1))
else
  _fail "validate_index fn missing from lynx-registry/index.rs"
fi

if rg -q "validate_index" "$FETCH_SRC" 2>/dev/null; then
  _ok "fetch pipeline calls validate_index"
  PASS=$((PASS + 1))
else
  _fail "fetch_plugin does not call validate_index — index integrity not enforced before install"
fi

# ── 5. Docs-command mismatch ─────────────────────────────────────────────────

_hdr "Docs-command mismatch"

README="$REPO_ROOT/README.md"
CLI_RS="$REPO_ROOT/crates/lynx-cli/src/cli.rs"

for cmd in "lx doctor" "lx plugin" "lx theme" "lx context" "lx init" "lx dashboard" "lx run" "lx jobs" "lx cron"; do
  if rg -q "$cmd" "$README" 2>/dev/null; then
    _ok "README documents: $cmd"
    PASS=$((PASS + 1))
  else
    _fail "README missing: $cmd"
  fi
done

for subcmd in Init Plugin Theme Context Doctor Daemon Config Migrate Update Rollback Sync Dashboard Jobs Run Cron; do
  if rg -q "\\b${subcmd}\\b" "$CLI_RS" 2>/dev/null; then
    _ok "cli.rs declares: $subcmd"
    PASS=$((PASS + 1))
  else
    _fail "cli.rs missing subcommand: $subcmd"
  fi
done

# ── 6. Cargo tests (registry checksum unit tests) ────────────────────────────

if [[ "${1:-}" != "--no-cargo" ]]; then
  _hdr "Cargo tests (lynx-registry)"
  if cargo nextest run -p lynx-registry --no-fail-fast 2>&1; then
    _ok "cargo nextest: lynx-registry — PASS"
    PASS=$((PASS + 1))
  else
    _fail "cargo nextest: lynx-registry — FAIL"
  fi
fi

# ── 7. Bats shell integration tests ──────────────────────────────────────────

if [[ "${1:-}" != "--no-bats" ]] && command -v bats >/dev/null 2>&1; then
  _hdr "Bats shell integration tests"
  if bats "$REPO_ROOT/tests/integration/shell/" 2>&1; then
    _ok "bats: all shell integration tests — PASS"
    PASS=$((PASS + 1))
  else
    _fail "bats: one or more shell integration tests — FAIL"
  fi
elif ! command -v bats >/dev/null 2>&1; then
  echo "  (skipping bats — not installed)"
fi

# ── 8. Env-var literal drift ─────────────────────────────────────────────────

_hdr "Env-var literal drift"

if bash "$REPO_ROOT/scripts/check-env-vars.sh" 2>&1; then
  PASS=$((PASS + 1))
else
  _fail "env-var literal drift detected — run scripts/check-env-vars.sh for details"
fi

# ── Summary ───────────────────────────────────────────────────────────────────

echo
echo "══════════════════════════════════════"
echo "  Guardrail results: ${PASS} passed, ${FAIL} failed"
echo "══════════════════════════════════════"

if [ "${FAIL}" -gt 0 ]; then
  echo
  echo "FAILURES:"
  for f in "${FAILURES[@]}"; do
    echo "  • $f"
  done
  echo
  exit 1
fi

echo "  All guardrails passed."
echo
exit 0
