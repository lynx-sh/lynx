# ADR-005: Git-Backed Config Sync (No Proprietary Cloud)

**Status:** Accepted

## Context
Power users have multiple machines. Config sync is needed. Cloud sync services add a dependency, a privacy concern, and ongoing maintenance.

## Decision
lx sync init turns ~/.config/lynx/ into a git repo. Users bring their own remote (GitHub, GitLab, private). Lynx wraps git push/pull/status. Secrets and snapshots are excluded via .gitignore.

## Consequences
- No cloud dependency or account required.
- Users control their own config data.
- Merge conflicts are possible — handled by standard git tooling.
- Sync is always explicit, never automatic.
