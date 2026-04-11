# node plugin — functions.zsh
# Reads .node-version or .nvmrc in the current directory.
# Never invokes nvm or any version manager. Fast, file-read only.

# Gather node version from project version files and export as cache env var.
# Called on chpwd and on plugin init. Unsets the var when no version file is found.
node_gather_state() {
  local ver=""
  if [[ -f "${PWD}/.node-version" ]]; then
    ver="${$(<"${PWD}/.node-version")//[$'\r\n']}"
  elif [[ -f "${PWD}/.nvmrc" ]]; then
    ver="${$(<"${PWD}/.nvmrc")//[$'\r\n']}"
  fi
  if [[ -n "$ver" ]]; then
    export LYNX_CACHE_NODE_STATE="{\"version\":\"${ver//\"/}\"}"
  else
    unset LYNX_CACHE_NODE_STATE
  fi
}
