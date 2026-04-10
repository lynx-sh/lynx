# ADR-002: No Oh My Zsh Compatibility Layer

**Status:** Accepted

## Context
OMZ plugins are convention-based (name it plugin.zsh) with no manifest, no deps, no lifecycle. Building a compat layer would permanently constrain Lynx's plugin architecture to OMZ's lack of one.

## Decision
No OMZ compatibility layer. OMZ plugins are mostly alias lists that are trivial to rewrite better. The two genuinely useful OMZ-adjacent projects (zsh-syntax-highlighting, zsh-autosuggestions) are standalone and load fine without any compat layer.

## Consequences
- Users cannot drop OMZ plugins directly into Lynx. They must use Lynx-native plugins.
- First-party plugins cover the most common use cases (git, fzf, kubectl, atuin).
- Plugin architecture is unconstrained by legacy conventions.
