# Lynx — AI Agent Skill

## Architecture
Rust workspace (`crates/`) → `lx` binary. Thin ~200-line zsh layer evals output from `lx`. All logic is in Rust. No logic in shell.

## Universal Execution Contract
Every task. No exceptions.
- **How to test:** `cargo nextest run --all` (final verification only — use `-p lynx-<crate>` during work)
- **Decisions gate code:** `pt decisions <component>` before implementing anything non-trivial
- **File issues proactively:** broken code, smells, missing tests → `pt add` immediately with severity
- **Pre-code checklist:** fill the relevant GATE below and emit it BEFORE writing any code. Blank fields = violation.

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
Clippy clean:           cargo clippy --all -- -D warnings — PASS (0 warnings)
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
G2 PRE-CODE AUDIT
Responsibility (D-042):
  For each file modified/created:
    File:           ____
    Single domain:  yes — "it does ____" / no — needs split first
    300+ lines:     yes/no — responsibility still singular: yes/no

Error paths (D-036):
  New user-facing errors: yes / none
  If yes — all use LynxError::____: yes / VIOLATION — bail!() is forbidden for user output
  Hint is actionable:     yes — "run lx ____ to ____"

List/browse output (D-040):
  Does this command output a list that will grow? yes / no
  If yes — uses InteractiveList TUI:              yes / VIOLATION — raw println! lists are forbidden
  TUI gate (do NOT add your own): lynx_tui::show() / show_multi() call gate::tui_enabled() internally.
  Gate checks: TTY + LYNX_NO_TUI + LYNX_CONTEXT=agent + CLAUDECODE + CURSOR_CLI + CI + config [tui] enabled.

Duplicate logic check:
  Functions being added: ____
  Grepped for similar: rg "fn_name\|pattern" crates/ — 0 existing / N existing → reuse ____

Silent failures:
  Error paths added: ____ — all propagated or logged: yes / VIOLATION — let _ = on fallible ops is forbidden
```
```
G3 IMPLEMENTATION
Files modified:     ____
Public API added:   ____ / none
Error paths:        LynxError::____ / none added
Tests added:        test_name — verifies ____
All tests pass:     cargo nextest run --all — PASS
Clippy clean:       cargo clippy --all -- -D warnings — PASS (0 warnings)
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
Clippy clean:           cargo clippy --all -- -D warnings — PASS (0 warnings)
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
8. **LynxError for all user-facing errors** (D-036) — never raw `bail!()` for user output. Every error the user sees MUST have a hint line telling them what to do next. Read `patterns/error-protocol.md`.
9. **Proactive issue filing** — broken code found during work → `pt add` immediately, never silently note and move on
10. **No production panics** — no `.unwrap()` or `.expect()` in non-test code unless compile-time guaranteed (static regex, const builder)
11. **TUI for all list output** (D-040) — any command that outputs a list which can grow must use `lynx_tui::InteractiveList`. Raw `println!` loops for list data are forbidden. The TUI gate (`lynx_tui::gate::tui_enabled`) handles all fallback automatically — TTY check, `LYNX_CONTEXT=agent`, `LYNX_NO_TUI`, `CLAUDECODE`, `CURSOR_CLI`, `CI`, and `[tui] enabled = false` in config. Never add your own TTY check.
12. **No duplicate logic** — before writing a function, grep for similar implementations. If it exists, reuse it. 4 copies of the same function = P2 issue.
13. **No silent failures** — `let _ =` on a fallible operation is forbidden unless the failure is genuinely irrelevant (and commented why). At minimum, `tracing::warn!` the failure.
14. **Zero clippy warnings** — `cargo clippy --all -- -D warnings` must pass before any commit. Warnings are not deferred. Baseline is always 0.

## Session Protocol
```bash
# START
pt go                          # mandatory — gets sitrep, handoff, P0s

# DURING
pt decisions <component>       # before any non-trivial design
cargo nextest run -p lynx-<X>  # targeted tests during work
# file issues as you find them — pt add

# END
cargo nextest run --all                    # final full suite — once only
cargo clippy --all -- -D warnings          # MUST be clean — 0 warnings, no exceptions
git commit                                 # commit before pt done
pt done S-XXX success "what was done" "what next agent does first"
```
