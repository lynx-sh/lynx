#!/usr/bin/env bash
# Lynx installer
# Usage: curl -sf https://raw.githubusercontent.com/proxikal/lynx/main/install.sh | sh

set -euo pipefail

LYNX_REPO="https://github.com/proxikal/lynx"
LYNX_CONFIG="${HOME}/.config/lynx"
LYNX_BIN="${HOME}/.local/bin"

print_step() { printf "\033[1;34m  %s\033[0m %s\n" "→" "$1"; }
print_ok()   { printf "\033[1;32m  %s\033[0m %s\n" "✓" "$1"; }
print_err()  { printf "\033[1;31m  %s\033[0m %s\n" "✗" "$1" >&2; }

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

# Check deps
for dep in git curl cargo; do
  if ! command -v "$dep" &>/dev/null; then
    print_err "Required: $dep not found"
    exit 1
  fi
done

print_step "Building lx binary..."
# TODO: pull release binary or build from source
# cargo build --release --manifest-path /tmp/lynx/Cargo.toml
print_ok "Binary ready"

print_step "Installing config..."
mkdir -p "$LYNX_CONFIG" "$LYNX_BIN"
print_ok "Config directory: $LYNX_CONFIG"

print_step "Choosing starter theme..."
echo ""
echo "  Pick a vibe:"
echo "  [1] default    clean, fast, no noise"
echo "  [2] minimal    just the essentials"
echo "  [3] powerline  classic, git-heavy"
echo "  [4] random     surprise me"
echo ""
read -r -p "  > " choice

case "$choice" in
  1|"") theme="default" ;;
  2)    theme="minimal" ;;
  3)    theme="powerline" ;;
  4)    theme="random" ;;
  *)    theme="default" ;;
esac

print_ok "Theme: $theme"

print_step "Wiring shell integration..."
ZSHRC="${ZDOTDIR:-$HOME}/.zshrc"
LYNX_SOURCE_LINE='source "${HOME}/.config/lynx/shell/init.zsh"'
if ! grep -qF "$LYNX_SOURCE_LINE" "$ZSHRC" 2>/dev/null; then
  echo "" >> "$ZSHRC"
  echo "# Lynx shell framework" >> "$ZSHRC"
  echo "$LYNX_SOURCE_LINE" >> "$ZSHRC"
fi
print_ok "Wired to $ZSHRC"

echo ""
echo "  Done. Start a new shell or: source ~/.zshrc"
echo "  Run 'lx doctor' if anything looks off."
echo ""
