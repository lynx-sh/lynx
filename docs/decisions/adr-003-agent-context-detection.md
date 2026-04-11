# ADR-003: Automatic Agent Context Detection via Environment Variables

**Status:** Accepted

## Context

AI coding agents (Claude Code, Cursor, GitHub Copilot CLI) run in a shell
environment. Shell frameworks typically cause problems for agents: aliases
shadow commands the agent expects to run directly, colorized output breaks
parsing, and prompt rendering adds latency and noise.

The naive fix is to tell users to add `if [[ -n "$CLAUDE_CODE" ]]; then ...`
in their `.zshrc`. This is brittle — users forget, the check must be maintained
per-agent, and it requires user action.

## Decision

Lynx detects agent context automatically at init time by checking well-known
environment variables:

| Env var | Set by |
|---|---|
| `CLAUDE_CODE=1` | Claude Code CLI |
| `CURSOR_SESSION=<session-id>` | Cursor |
| `CI=true` | CI systems (triggers minimal — non-interactive, not agent) |
| `LYNX_CONTEXT=minimal` | Users who want minimal mode manually |

When any of these is set, Lynx sets `LYNX_CONTEXT=agent` (or `minimal`).
Plugins with `disabled_in = ["agent"]` are skipped. Aliases are never loaded.

**No user action required.** Installing Lynx is sufficient — agent context
is automatic.

## Consequences

**Positive:**
- Agents get a clean shell environment without user configuration
- Works for Claude Code, Cursor, and CI out of the box
- Plugin authors declare `disabled_in = ["agent"]` once and it just works

**Negative:**
- New agents not in the detection list need an explicit env var to be added
- Detection happens at init — context cannot change mid-session

**Invariant this creates:**
- D-004: Agent context detection is automatic — no manual context-setting code
- D-010: All plugins must declare `disabled_in = ["agent", "minimal"]` for aliases
