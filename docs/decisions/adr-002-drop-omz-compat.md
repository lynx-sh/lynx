# ADR-002: No Oh-My-Zsh Compatibility Layer

**Status:** Accepted

## Context

Oh-My-Zsh (OMZ) has a large plugin and theme ecosystem. There was a request
to make Lynx compatible with OMZ plugins so users could migrate without
rewriting their plugins.

This would require: detecting OMZ plugin formats, sourcing OMZ's plugin loader,
emulating OMZ's theme API (`$PROMPT`, `$ZSH_THEME_GIT_PROMPT_*`, etc.), and
maintaining that compatibility surface indefinitely.

## Decision

Lynx does not support OMZ plugins or themes. There is no compatibility shim,
no OMZ emulation layer, and no plan to add one.

Lynx plugins use `plugin.toml` manifests and a well-defined export convention.
OMZ plugins are arbitrary zsh scripts with no manifest — they cannot be loaded
safely without executing arbitrary code.

Users migrating from OMZ should rewrite their plugins using `lx plugin new`.
The authoring guide provides a full migration path.

## Consequences

**Positive:**
- No undefined behavior from running untrusted OMZ plugin code in Lynx's context
- Plugin isolation (namespace lint, context filter) works reliably
- No maintenance burden for a compatibility surface we don't control
- Lynx plugins are auditable — manifest declares exactly what they export

**Negative:**
- Migration cost for OMZ users — plugins must be rewritten
- Smaller initial plugin ecosystem

**Invariant this creates:**
- D-002: No OMZ compatibility layer — `lx plugin add` only accepts Lynx-format plugins
