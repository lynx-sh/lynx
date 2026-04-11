# golang plugin — functions.zsh
# Reads go.mod to extract the `go X.Y` directive.
# Never invokes go toolchain or any version manager. Fast, grep-based.

golang_gather_state() {
  local ver=""
  if [[ -f "${PWD}/go.mod" ]]; then
    # Extract version from line like: `go 1.22.3` or `toolchain go1.22.3`
    ver=$(command grep -m1 '^go ' "${PWD}/go.mod" 2>/dev/null | awk '{print $2}')
  fi
  if [[ -n "$ver" ]]; then
    export LYNX_CACHE_GOLANG_STATE="{\"version\":\"${ver//\"/}\"}"
  else
    unset LYNX_CACHE_GOLANG_STATE
  fi
}
