# Shell Layer Protocol

## The Prime Rule

**The shell/ directory contains zero logic.**

If you find yourself writing a conditional, a loop, a calculation, or any
non-trivial string manipulation in a shell/ file: STOP. Move it to Rust.

## What IS Allowed in shell/

```zsh
# Allowed: sourcing other files
source "${LYNX_DIR}/shell/core/hooks.zsh"

# Allowed: eval of lx binary output
eval "$(lx init --context "${_lynx_context}")"

# Allowed: env var assignment (simple, no computation)
export LYNX_CONTEXT="interactive"

# Allowed: add-zsh-hook registration
add-zsh-hook chpwd _lynx_hook_chpwd

# Allowed: simple forwarding function (under 3 lines)
_lynx_hook_chpwd() { lx event emit "shell:chpwd" --data "$PWD" 2>/dev/null }
```

## What is NOT Allowed in shell/

```zsh
# NOT ALLOWED: conditionals with logic
if [[ "$SOME_CONDITION" ]]; then
  # compute something
fi

# NOT ALLOWED: loops
for plugin in "${plugins[@]}"; do
  source "$plugin"
done

# NOT ALLOWED: string manipulation beyond simple assignment
local result="${var/pattern/replacement}"

# NOT ALLOWED: arithmetic
((count++))
```

## File Size Limits

| File | Max Lines |
|---|---|
| shell/core/loader.zsh | 30 |
| shell/core/hooks.zsh | 40 |
| shell/lib/eval-bridge.zsh | 30 |
| shell/init.zsh | 10 |
| plugins/*/shell/init.zsh | 10 |
| plugins/*/shell/functions.zsh | 100 |
| plugins/*/shell/aliases.zsh | 30 |

Exceeding limits = the file is doing too much. Move logic to Rust.

## Silence on Failure

Every lx call in shell/ must be silent on failure:
```zsh
lx event emit "shell:chpwd" 2>/dev/null   # CORRECT
lx event emit "shell:chpwd"                # WRONG — error visible to user
```

A broken lx binary must never break the shell. Hooks and bridges are always optional.

## Testing Shell Files

```bash
# Syntax check
zsh -n shell/core/loader.zsh

# Source in clean subshell
env -i HOME=/tmp/test zsh -c "source $(pwd)/shell/core/loader.zsh"

# Verify line counts
wc -l shell/**/*.zsh plugins/**/shell/*.zsh
```
