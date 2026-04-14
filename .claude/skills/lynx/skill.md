# Lynx — AI Agent Skill

## Architecture
Rust workspace (`crates/`) → `lx` binary. ~200-line zsh layer evals `lx` output. All logic is Rust.

## Workflow Gates
| Gate | Command | When |
|------|---------|------|
| Final verify | `lx run lynx-ai-verify` | End of every session |
| Shell verify | `lx run lynx-ai-shell` | Any shell/ or plugin shell change |
| Targeted test | `cargo nextest run -p lynx-<crate>` | During work, crate under change |

## Task Router

| Task | Mandatory pattern file |
|------|----------------------|
| shell/ or plugin shell change | `shell-protocol.md` |
| New/changed user-facing error | `error-protocol.md` |
| New crate or cross-crate dep | `crate-protocol.md` |
| Plugin work | `plugin-protocol.md` |
| Block/phase scaffolding | `phase-protocol.md` |
| Bug fix / feature / refactor (none of above) | none |

---

## GATE: BUG FIX

```
G1 SCOPE
Issue:            H-XXX
Patterns read:    none / <file>.md — confirmed
Grep:             rg "____" crates/ — N instances across M files
Files affected:   path/to/file.rs, ...
All fixed:        yes / no — remainder filed as H-XXX
```
```
G2 FIX
Root cause:       ____
Fix location:     file.rs:fn_name
Regression test:  test_name / impossible because: ____
```
```
G3 AUDIT
Re-grep:          rg "____" crates/ — 0 remaining
Verify:           lx run lynx-ai-verify — PASS
```
`pt fix H-XXX "cause" "fix" "scope: grepped X, N sites, all fixed"`

---

## GATE: FEATURE

```
G1 DESIGN
Feature:          ____
Patterns read:    none / <file>.md — confirmed
Crates touched:   lynx-____
New crate:        no / yes — justified: ____
Cross-crate deps: none / lynx-X → lynx-Y
```
```
G2 PRE-CODE AUDIT
Responsibility:   each file does one thing — yes / SPLIT: <file>
Error paths:      none / LynxError::<Variant> — no bare bail!() for user errors
List output:      no / lynx_tui::show() — no raw println! for list data
Panics:           none / compile-time safe: <justification>
Silent failures:  none / tracing::warn! on each fallible op
```
```
G3 IMPLEMENTATION
Files modified:   ____
Tests added:      test_name — verifies ____
Verify:           lx run lynx-ai-verify — PASS
```

---

## GATE: REFACTOR

```
G1 SCOPE
Files:            ____
Split rationale:  "file does A and B" → ____
Patterns read:    none / <file>.md — confirmed
Behavior change:  NONE
```
```
G2 REFACTOR
New files:        ____ — each owns: ____
mod.rs/lib.rs:    updated
use paths:        updated — files: ____
Public API:       unchanged / intentional changes: ____
Verify:           lx run lynx-ai-verify — PASS
```
Bugs found during refactor → `pt add`, fix separately.

---

## Hard Rules
1. No logic in shell/ — Rust only (D-001) → `shell-protocol.md`
2. Config is TOML — never zsh as config (D-003)
3. Aliases are context-gated — never unconditional
4. Snapshot before config mutation (D-007)
5. Single-responsibility files (D-042) — "it does X and Y" = split
6. `pt decisions <component>` before non-trivial design
7. LynxError for all user-facing errors (D-036) → `error-protocol.md`
8. No `.unwrap()`/`.expect()` in non-test code unless compile-time safe
9. No `let _ =` on fallible ops — at minimum `tracing::warn!`
10. Zero clippy warnings, fmt drift, shell violations — all enforced by `lx run lynx-ai-verify`

## Session Protocol
```bash
pt go                              # START — mandatory
pt decisions <component>           # before any non-trivial design
cargo nextest run -p lynx-<crate>  # targeted tests during work
lx run lynx-ai-verify              # END — final gate (MUST pass before any push)
git commit && pt done S-XXX success "what was done" "what next agent does first"
```

> **Push gate (non-negotiable):** `lx run lynx-ai-verify` must exit 0 before any `git push`.
> It runs fmt, clippy, all Rust tests, all bats shell tests, and guardrails — the same checks CI enforces.
