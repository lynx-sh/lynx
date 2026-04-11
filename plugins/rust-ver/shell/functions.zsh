# rust-ver plugin — functions.zsh
# Reads rust-toolchain.toml (channel field) or legacy rust-toolchain file.
# Never invokes rustup or cargo. Fast, grep/awk-based.

rust_ver_gather_state() {
  local ver=""
  if [[ -f "${PWD}/rust-toolchain.toml" ]]; then
    # Extract: channel = "stable" or channel = "1.78.0"
    ver=$(command grep -m1 '^channel' "${PWD}/rust-toolchain.toml" 2>/dev/null \
      | awk -F'"' '{print $2}')
  elif [[ -f "${PWD}/rust-toolchain" ]]; then
    ver="${$(<"${PWD}/rust-toolchain")//[$'\r\n']}"
  fi
  if [[ -n "$ver" ]]; then
    export LYNX_CACHE_RUST_STATE="{\"version\":\"${ver//\"/}\"}"
  else
    unset LYNX_CACHE_RUST_STATE
  fi
}
