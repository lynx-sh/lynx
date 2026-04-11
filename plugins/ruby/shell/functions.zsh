# ruby plugin — functions.zsh
# Reads .ruby-version in the current directory.
# Never invokes rvm, rbenv, or any version manager. Fast, file-read only.

ruby_gather_state() {
  local ver=""
  if [[ -f "${PWD}/.ruby-version" ]]; then
    ver="${$(<"${PWD}/.ruby-version")//[$'\r\n']}"
  fi
  if [[ -n "$ver" ]]; then
    export LYNX_CACHE_RUBY_STATE="{\"version\":\"${ver//\"/}\"}"
  else
    unset LYNX_CACHE_RUBY_STATE
  fi
}
