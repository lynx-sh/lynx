# ADR-005: Git-Backed Config Sync

**Status:** Accepted

## Context

Users want to sync their Lynx config across machines. Cloud sync services
(Dropbox, iCloud Drive) cause race conditions on TOML files. Custom sync
protocols require servers. The simplest reliable distributed store that
developers already use is git.

## Decision

Lynx config sync is implemented as git operations on `~/.config/lynx/`.
The directory is a git repository (or can be initialized as one). The `lx sync`
command wraps git commit, push, and pull.

```bash
lx sync init          # git init ~/.config/lynx, set remote
lx sync push          # commit and push latest config
lx sync pull          # pull and apply remote config
lx sync status        # show uncommitted changes
```

Conflict resolution uses standard git merge. If merge fails, the user resolves
it with standard git tools — no custom merge driver.

## Consequences

**Positive:**
- Full history of config changes — rollback to any point
- No sync server to maintain
- Works with any git remote (GitHub, GitLab, self-hosted)
- Merge conflicts use the same tools developers already know

**Negative:**
- Requires git on the machine (nearly universal for Lynx's target users)
- Users must set up a remote manually — no zero-config cloud sync
- Large binary assets in config (e.g., fonts) would bloat the git repo
  (documented constraint: keep themes and profiles text-only)

**Constraint this creates:**
- All config files must be valid TOML — binary blobs in config dir are unsupported
- Themes and profiles should not reference absolute paths (breaks cross-machine sync)
