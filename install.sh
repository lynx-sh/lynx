#!/usr/bin/env bash
# Lynx installer
# Usage: curl -sf https://raw.githubusercontent.com/proxikal/lynx/main/install.sh | sh
#
# Installs lx to ~/.local/bin and shell integration to ~/.config/lynx.
# Requires: cargo (to build), or a pre-built release binary.
# Does NOT require root.

set -euo pipefail

LYNX_CONFIG="${HOME}/.config/lynx"
LYNX_BIN_DIR="${HOME}/.local/bin"
LYNX_BIN="${LYNX_BIN_DIR}/lx"
ZSHRC="${ZDOTDIR:-$HOME}/.zshrc"
LYNX_SOURCE_LINE='source "${HOME}/.config/lynx/shell/init.zsh"'

# ── Colours ──────────────────────────────────────────────────────────────────
RED='\033[1;31m'; GRN='\033[1;32m'; BLU='\033[1;34m'; RST='\033[0m'
step() { printf "${BLU}  → ${RST}%s\n" "$1"; }
ok()   { printf "${GRN}  ✓ ${RST}%s\n" "$1"; }
err()  { printf "${RED}  ✗ ${RST}%s\n" "$1" >&2; }
warn() { printf "${RED}  ! ${RST}%s\n" "$1"; }

# ── Banner ────────────────────────────────────────────────────────────────────
echo ""
echo "  ██╗  ██╗   ██╗███╗  ██╗██╗  ██╗"
echo "  ██║  ╚██╗ ██╔╝████╗ ██║╚██╗██╔╝"
echo "  ██║   ╚████╔╝ ██╔██╗██║ ╚███╔╝ "
echo "  ██║    ╚██╔╝  ██║╚████║ ██╔██╗ "
echo "  ███████╗██║   ██║ ╚███║██╔╝╚██╗"
echo "  ╚══════╝╚═╝   ╚═╝  ╚══╝╚═╝  ╚═╝"
echo ""
echo "  The shell framework that doesn't suck."
echo ""

# ── Preflight ─────────────────────────────────────────────────────────────────
# Detect if we're running from the repo dir (developer install) or not.
REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" 2>/dev/null && pwd || echo "")"
IN_REPO=false
if [[ -f "${REPO_DIR}/Cargo.toml" ]] && grep -q 'name = "lynx-cli"' "${REPO_DIR}/Cargo.toml" 2>/dev/null; then
  IN_REPO=true
fi

if ! command -v cargo &>/dev/null && [[ "$IN_REPO" == true ]]; then
  err "cargo not found — needed to build lx from source."
  echo "  Install Rust: https://rustup.rs"
  exit 1
fi

# ── Build / acquire binary ────────────────────────────────────────────────────
mkdir -p "$LYNX_BIN_DIR"

if [[ "$IN_REPO" == true ]]; then
  step "Building lx from source (this takes ~30s first time)..."
  cargo build --release --manifest-path "${REPO_DIR}/Cargo.toml" --quiet
  cp "${REPO_DIR}/target/release/lx" "$LYNX_BIN"
  chmod +x "$LYNX_BIN"
  ok "Binary installed: $LYNX_BIN"
elif command -v lx &>/dev/null; then
  ok "lx binary already on PATH — skipping build"
else
  err "lx binary not found and not running from the repo."
  echo "  Clone the repo and run install.sh from there:"
  echo "    git clone https://github.com/proxikal/lynx && cd lynx && ./install.sh"
  exit 1
fi

# Ensure ~/.local/bin is on PATH for this session
export PATH="${LYNX_BIN_DIR}:${PATH}"

# ── Verify binary works ───────────────────────────────────────────────────────
if ! "$LYNX_BIN" --version &>/dev/null; then
  err "lx binary failed to run. Try: $LYNX_BIN --version"
  exit 1
fi

# ── Config directory ──────────────────────────────────────────────────────────
step "Setting up config directory..."
mkdir -p "${LYNX_CONFIG}/shell/core"
mkdir -p "${LYNX_CONFIG}/shell/lib"
mkdir -p "${LYNX_CONFIG}/themes"
mkdir -p "${LYNX_CONFIG}/contexts"
mkdir -p "${LYNX_CONFIG}/plugins"

# Copy shell integration files from repo (if in repo) or skip.
if [[ "$IN_REPO" == true ]]; then
  cp -r "${REPO_DIR}/shell/." "${LYNX_CONFIG}/shell/"
  cp -r "${REPO_DIR}/themes/." "${LYNX_CONFIG}/themes/"
  cp -r "${REPO_DIR}/contexts/." "${LYNX_CONFIG}/contexts/"
fi
ok "Config directory: $LYNX_CONFIG"

# ── Default config.toml ───────────────────────────────────────────────────────
CONFIG_FILE="${LYNX_CONFIG}/config.toml"
if [[ -f "$CONFIG_FILE" ]]; then
  warn "Existing config found at $CONFIG_FILE — skipping (use lx config to edit)"
else
  # Theme selection
  echo ""
  echo "  Pick a starter theme:"
  echo "  [1] default    clean, fast, no noise"
  echo "  [2] minimal    just the essentials"
  echo ""
  if [[ -t 0 ]]; then
    read -r -p "  > " choice
  else
    choice="1"
  fi

  case "$choice" in
    2) chosen_theme="minimal" ;;
    *) chosen_theme="default" ;;
  esac

  cat > "$CONFIG_FILE" <<TOML
schema_version = 1
active_theme   = "${chosen_theme}"
active_context = "interactive"
enabled_plugins = []
TOML
  ok "Config written: $CONFIG_FILE (theme: $chosen_theme)"
fi

# ── Shell integration — .zshrc wiring (idempotent) ────────────────────────────
step "Wiring shell integration..."
touch "$ZSHRC"

if grep -qF "$LYNX_SOURCE_LINE" "$ZSHRC" 2>/dev/null; then
  ok "Shell integration already in $ZSHRC — no change"
else
  {
    echo ""
    echo "# Lynx shell framework"
    echo "$LYNX_SOURCE_LINE"
  } >> "$ZSHRC"
  ok "Added to $ZSHRC"
fi

# ── PATH wiring (idempotent) ───────────────────────────────────────────────────
PATH_LINE='export PATH="${HOME}/.local/bin:${PATH}"'
if ! grep -qF "$PATH_LINE" "$ZSHRC" 2>/dev/null; then
  {
    echo ""
    echo "# Add ~/.local/bin to PATH (for lx and other tools)"
    echo "$PATH_LINE"
  } >> "$ZSHRC"
  ok "Added ~/.local/bin to PATH in $ZSHRC"
fi

# ── Validate with lx doctor ───────────────────────────────────────────────────
step "Running lx doctor..."
if LYNX_DIR="$LYNX_CONFIG" "$LYNX_BIN" doctor 2>/dev/null; then
  ok "lx doctor passed"
else
  warn "lx doctor reported some issues — run 'lx doctor' after restarting your shell"
fi

# ── Done ─────────────────────────────────────────────────────────────────────
echo ""
echo "  ✓ Lynx is installed."
echo ""
echo "  Next steps:"
echo "    1. Start a new shell, or run:  source ~/.zshrc"
echo "    2. Check your setup:           lx doctor"
echo "    3. Browse themes:              lx theme list"
echo "    4. Add a plugin:               lx plugin add ./plugins/<name>"
echo "    5. See examples:               lx examples"
echo ""
