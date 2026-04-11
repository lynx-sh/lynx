# kubectl plugin — functions.zsh
# State is cached into _lynx_kubectl_state[] on chpwd hook.
# The kubectl_context prompt segment reads from this cache.

typeset -gA _lynx_kubectl_state

# Refresh kubectl state cache — called on chpwd hook
kubectl_refresh_state() {
  if ! command -v kubectl &>/dev/null; then
    _lynx_kubectl_state=()
    return 0
  fi
  if [[ -z "$KUBECONFIG" && ! -f "$HOME/.kube/config" ]]; then
    _lynx_kubectl_state=()
    return 0
  fi

  local ctx ns
  ctx=$(kubectl config current-context 2>/dev/null)
  ns=$(kubectl config view --minify --output 'jsonpath={..namespace}' 2>/dev/null)
  ns="${ns:-default}"

  _lynx_kubectl_state=(context "$ctx" namespace "$ns")
}

# Read helpers — fast, no subprocess
kubectl_current_context()   { echo "${_lynx_kubectl_state[context]}" }
kubectl_current_namespace() { echo "${_lynx_kubectl_state[namespace]:-default}" }

# fzf-powered context switcher
kctx() {
  if ! command -v fzf &>/dev/null; then
    echo "kctx: fzf required — brew install fzf" >&2
    return 1
  fi
  local ctx
  ctx=$(kubectl config get-contexts --no-headers -o name 2>/dev/null | fzf --prompt="context> " --height=40%)
  [[ -n "$ctx" ]] && kubectl config use-context "$ctx" && kubectl_refresh_state
}

# fzf-powered namespace switcher
kns() {
  if ! command -v fzf &>/dev/null; then
    echo "kns: fzf required — brew install fzf" >&2
    return 1
  fi
  local ns
  ns=$(kubectl get namespaces --no-headers -o custom-columns=':metadata.name' 2>/dev/null \
    | fzf --prompt="namespace> " --height=40%)
  if [[ -n "$ns" ]]; then
    local ctx="${_lynx_kubectl_state[context]}"
    kubectl config set-context "$ctx" --namespace="$ns" 2>/dev/null
    kubectl_refresh_state
  fi
}

_kubectl_plugin_chpwd() { kubectl_refresh_state }
