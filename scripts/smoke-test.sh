#!/usr/bin/env zsh
# Lynx end-to-end smoke test — runs non-interactively, suitable for CI.
# Exits 0 on success, 1 on any failure.

set -euo pipefail

SCRIPT_DIR="${0:A:h}"
WORKSPACE_ROOT="${SCRIPT_DIR}/.."
TMPDIR_ROOT="$(mktemp -d)"

cleanup() {
  rm -rf "$TMPDIR_ROOT"
}
trap cleanup EXIT

FAKE_HOME="${TMPDIR_ROOT}/home"
LYNX_INSTALL_DIR="${FAKE_HOME}/.config/lynx"
mkdir -p "$FAKE_HOME"

print_step() {
  print -- "  [smoke] $1"
}

fail() {
  print -- "  [FAIL] $1" >&2
  exit 1
}

print -- ""
print -- "Lynx smoke test"
print -- "==============="

# ── 1. lx binary is on PATH ────────────────────────────────────────────────
print_step "lx binary found"
if ! command -v lx &>/dev/null; then
  fail "lx not found on PATH — run: cargo install --path crates/lynx-cli"
fi

# ── 2. lx install copies files ────────────────────────────────────────────
print_step "lx install --source $WORKSPACE_ROOT --dir $LYNX_INSTALL_DIR"
lx install --source "$WORKSPACE_ROOT" --dir "$LYNX_INSTALL_DIR" \
  || fail "lx install exited non-zero"

[[ -f "${LYNX_INSTALL_DIR}/shell/core/hooks.zsh" ]] \
  || fail "hooks.zsh not installed"

[[ -f "${LYNX_INSTALL_DIR}/config.toml" ]] \
  || fail "config.toml not written"

# ── 3. hooks.zsh passes syntax check ─────────────────────────────────────
print_step "zsh -n hooks.zsh"
zsh -n "${LYNX_INSTALL_DIR}/shell/core/hooks.zsh" \
  || fail "hooks.zsh failed syntax check"

# ── 4. lx init produces valid zsh ─────────────────────────────────────────
print_step "lx init --context interactive"
export LYNX_DIR="$LYNX_INSTALL_DIR"
init_output="$(lx init --context interactive 2>/dev/null)"
[[ -n "$init_output" ]] || fail "lx init produced no output"
print -- "$init_output" | zsh -n || fail "lx init output failed zsh syntax check"

# ── 5. lx prompt render produces PROMPT= ──────────────────────────────────
print_step "lx prompt render"
export LYNX_CONTEXT=interactive
unset LYNX_CACHE_GIT_STATE 2>/dev/null || true
prompt_output="$(lx prompt render 2>/dev/null)"
[[ "$prompt_output" == *"PROMPT="* ]] || fail "lx prompt render did not produce PROMPT="

# ── 6. lx install --zshrc patches .zshrc ──────────────────────────────────
print_step "lx install --zshrc patches .zshrc"
FAKE_ZSHRC="${FAKE_HOME}/.zshrc"
HOME="$FAKE_HOME" lx install --source "$WORKSPACE_ROOT" --dir "$LYNX_INSTALL_DIR" --zshrc \
  || fail "lx install --zshrc exited non-zero"
grep -q 'eval "$(lx init)"' "$FAKE_ZSHRC" \
  || fail ".zshrc not patched with eval line"

# ── 7. Idempotency: run install again, still only one eval line ───────────
print_step "install idempotency"
HOME="$FAKE_HOME" lx install --source "$WORKSPACE_ROOT" --dir "$LYNX_INSTALL_DIR" --zshrc \
  2>/dev/null || true
count=$(grep -c 'eval "$(lx init)"' "$FAKE_ZSHRC")
[[ "$count" -eq 1 ]] || fail ".zshrc has $count eval lines (expected 1)"

print -- ""
print -- "All smoke tests passed."
print -- ""
