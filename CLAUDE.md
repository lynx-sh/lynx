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
crates/         Rust workspace (see maps/crate-deps.md for dep rules)
shell/          Thin zsh integration layer (logic-free — see patterns/shell-protocol.md)
plugins/        First-party plugins (see patterns/plugin-protocol.md)
themes/         TOML theme files
contexts/       Context configs (interactive, agent, minimal)
profiles/       Named load profiles
docs/           Architecture, guides, ADRs
tests/          Rust integration tests + bats shell tests
.claude/skills/lynx/   Skill system (maps + patterns — read when task router points here)
```

## Skill System (lazy-loaded)

Read these only when the task router in skill.md points to them:

**Maps** (read before crossing these boundaries):
- `maps/crate-deps.md` — before adding a crate or cross-crate dependency
- `maps/shell-flow.md` — before touching shell/ or lx init
- `maps/plugin-lifecycle.md` — before touching plugin loading or lifecycle
- `maps/event-flow.md` — before touching events, hooks, or IPC

**Patterns** (read when the gate says to):
- `patterns/testing.md` — test requirements and how to run them
- `patterns/plugin-protocol.md` — building or modifying plugins
- `patterns/phase-protocol.md` — working phases, filing issues
- `patterns/shell-protocol.md` — what is and is not allowed in shell/
- `patterns/crate-protocol.md` — crate structure, naming, file limits

## Key Commands

```bash
pt go                          # start every session
pt orient                      # cold-start: see all blocks, P0s, decisions
pt decisions arch              # non-negotiable architecture rules
pt decisions plugins           # plugin rules
pt decisions config            # config rules
cargo nextest run --all        # run all tests
bats tests/integration/shell/  # run shell integration tests
```

## Non-Negotiable Rules (summary — full list in skill.md Hard Rules)

1. No logic in shell/ — logic belongs in Rust
2. All config is TOML — never zsh code as config
3. Aliases are always context-gated — never unconditional
4. Snapshot before every config mutation
5. Secret redaction on all user-facing output that may include config values
6. File limit: 300 lines warning, 500 lines violation — split the file
7. `pt check-decision` before implementing any non-trivial design
