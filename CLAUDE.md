# Lynx — AI Agent Instructions

## FIRST ACTION (mandatory, no exceptions)

Read `.claude/skills/lynx/skill.md` before doing ANYTHING else.
That file contains the task router, gates, hard rules, and session protocol.
Proceeding without reading it = operating blind.

## What This Project Is

Lynx is a Rust-powered zsh shell framework. The `lx` CLI binary is the core.
A thin ~200-line zsh layer evals output from `lx`. All logic is in Rust crates.

## Project Layout

```
crates/         Rust workspace (crate deps → patterns/crate-protocol.md)
shell/          Thin zsh integration layer (logic-free → patterns/shell-protocol.md)
plugins/        First-party plugins (→ patterns/plugin-protocol.md)
themes/         TOML theme files
contexts/       Context configs (interactive, agent, minimal)
tests/          Rust integration tests + bats shell tests
.claude/skills/lynx/   Skill system (patterns — read when task router points here)
```

## Skill System (lazy-loaded)

Read these only when the task router in skill.md points to them:

**Patterns** (read when the gate says to):
- `patterns/testing.md` — test requirements and how to run them
- `patterns/error-protocol.md` — LynxError variants and user-facing error rules
- `patterns/plugin-protocol.md` — building or modifying plugins
- `patterns/phase-protocol.md` — working phases, filing issues
- `patterns/shell-protocol.md` — what is and is not allowed in shell/
- `patterns/crate-protocol.md` — crate structure, dependencies, file organization

## Key Commands

```bash
pt go                          # start every session
pt orient                      # cold-start: see all blocks, P0s, decisions
pt decisions <component>       # check rules before implementing
cargo nextest run --all        # run all tests (final verification only)
bats tests/integration/shell/  # run shell integration tests
```

## Non-Negotiable Rules (summary — full list in skill.md Hard Rules)

1. No logic in shell/ — logic belongs in Rust (D-001)
2. All config is TOML — never zsh code as config (D-003)
3. Aliases are always context-gated — never unconditional
4. Snapshot before every config mutation (D-007)
5. Secret redaction on all user-facing output that may include config values
6. Single-responsibility files — one domain per file, no hard line ceiling (D-042)
7. `pt decisions <component>` before implementing any non-trivial design
8. LynxError for all user-facing errors (D-036)
9. No production panics — no `.unwrap()` in non-test code unless compile-time safe
10. Proactive issue filing — find broken code → `pt add` immediately
11. **Push gate** — run `bash scripts/verify-guardrails.sh` AND `lx run lynx-ai-verify` before any `git push`. CI enforces the same checks and will reject branches that skip them.
