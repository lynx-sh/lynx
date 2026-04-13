# Lynx — AI Agent Skill

## Architecture
Rust workspace (`crates/`) → `lx` binary. Thin ~200-line zsh layer evals output from `lx`. All logic is in Rust. No logic in shell.

## Universal Execution Contract
Every task. No exceptions.
- **How to test:** `cargo nextest run --all` (final verification only — use `-p lynx-<crate>` during work)
- **Decisions gate code:** `pt decisions <component>` before implementing anything non-trivial
- **File issues proactively:** broken code, smells, missing tests → `pt add` immediately with severity

## Task Router

| Task | Gate |
|------|------|
| Bug fix | GATE: BUG FIX below |
| Feature / new code | GATE: FEATURE below |
| Refactor / file split | GATE: REFACTOR below |
| Any change to `shell/` | `patterns/shell-protocol.md` — no logic in shell, ever |
| Any change to error paths | `patterns/error-protocol.md` — LynxError variants only |
| New crate or cross-crate dep | `patterns/crate-protocol.md` |
| Plugin work | `patterns/plugin-protocol.md` |
| Testing questions | `patterns/testing.md` |
| Block/phase scaffolding | `patterns/phase-protocol.md` |

Gates are fill-in-the-blank audit trails. Copy, fill every field, emit before writing code. Blank = violation.

---

## GATE: BUG FIX

```
G1 SCOPE
Issue:              H-XXX
Decisions checked:  pt decisions <component> — relevant: D-XXX / none
Grep pattern:       rg "____" crates/
Instances found:    N across M files
Files affected:     path/to/file.rs, ...
All fixed here:     yes / no — if no: filed as H-XXX
```
```
G2 FIX
Root cause:         ____
Fix location(s):    file.rs — function/block
Dead code deleted:  none / list
Regression test:    test_name — verifies ____
                    (impossible because: ____ — MUST justify)
```
```
G3 SCOPE AUDIT
Re-grep:                rg "____" crates/
Remaining instances:    0 / N — if N > 0: filed as H-XXX
Bug class investigated: "pattern X" — grepped Y, found N sites → filed / all fixed
All tests pass:         cargo nextest run --all — PASS
```
`pt fix H-XXX "cause" "fix" "scope: grepped X in crates/, N sites, all fixed"`

---

## GATE: FEATURE

```
G1 DESIGN
Feature:            ____
Decisions checked:  pt decisions <component> — relevant: D-XXX / none
Crates touched:     lynx-____
New crate needed:   no / yes — justified: ____
Cross-crate deps:   none / lynx-X → lynx-Y — checked crate-protocol.md
```
```
G2 RESPONSIBILITY CHECK (D-042)
For each file being modified or created:
  File:             ____
  Single domain:    yes — "it does ____" / no — needs split: ____
  300+ lines after: yes/no — responsibility still singular: yes/no
New files created:  ____ — each owns exactly one domain: ____
```
```
G3 IMPLEMENTATION
Files modified:     ____
Public API added:   ____ / none
Error paths:        LynxError::____ / none added
Tests added:        test_name — verifies ____
All tests pass:     cargo nextest run --all — PASS
```

---

## GATE: REFACTOR

```
G1 SCOPE
Files to refactor:      ____
Responsibility audit:   "file X does A and B" — split into ____
Behavior changes:       NONE (refactor = structure only)
```
```
G2 REFACTOR
New files created:      ____ — each owns: ____
mod.rs / lib.rs updated: yes
use paths updated:      yes — files: ____
Public API unchanged:   yes / list intentional changes
All tests pass:         cargo nextest run --all — PASS
```
Behavioral bugs found during refactor → `pt add`, fix separately. Never mix.

---

## Hard Rules
1. **No logic in shell/** — logic belongs in Rust (D-001)
2. **All config is TOML** — never zsh code as config (D-003)
3. **Aliases are context-gated** — never unconditional
4. **Snapshot before config mutation** (D-007)
5. **Secret redaction** on all user-facing output that may include config values
6. **Single-responsibility files** (D-042) — one domain per file, no hard line ceiling. 300 lines = check responsibility, not mandate a split
7. **`pt decisions <component>`** before implementing non-trivial design
8. **LynxError for all user-facing errors** (D-036) — never raw `bail!()` for user output
9. **Proactive issue filing** — broken code found during work → `pt add` immediately, never silently note and move on
10. **No production panics** — no `.unwrap()` or `.expect()` in non-test code unless the value is compile-time guaranteed (static regex, const builder). Use `?`, `.ok()`, `.unwrap_or_default()`, or `.unwrap_or_else(|e| e.into_inner())` for mutex locks

## Session Protocol
```bash
# START
pt go                          # mandatory — gets sitrep, handoff, P0s

# DURING
pt decisions <component>       # before any non-trivial design
cargo nextest run -p lynx-<X>  # targeted tests during work
# file issues as you find them — pt add

# END
cargo nextest run --all        # final full suite — once only
git commit                     # commit before pt done
pt done S-XXX success "what was done" "what next agent does first"
```
