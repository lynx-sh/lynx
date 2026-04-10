# ADR-003: First-Class Agent Context for AI-Assisted Development

**Status:** Accepted

## Context
Aliases and aggressive completions interfere with AI agentic coding tools (Claude Code, Cursor, Copilot). A `git` alias that expands to something else breaks tool calls. Shell frameworks have no concept of this.

## Decision
Lynx has a built-in `agent` context that auto-detects AI tool sessions via env vars (CLAUDE_CODE, CURSOR_SESSION) and loads a minimal plugin set with aliases disabled. Plugins declare which contexts they're disabled in via plugin.toml.

## Consequences
- AI tools get a clean, predictable shell environment.
- Users don't need to manually disable plugins when switching to agentic work.
- Plugin authors must explicitly opt aliases into contexts.
