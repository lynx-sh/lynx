# Lynx — AI Agent Skill

## What Lynx Is
A Rust-powered zsh shell framework. Rust crates own all logic. A thin zsh layer (~200 lines) evals output from the `lx` binary. No OMZ compatibility. No pure-zsh logic outside the shell/ directory.

## Architecture Map
Before touching any cross-boundary code, read the relevant map:
- `maps/crate-deps.md` — allowed crate dependency directions (violations are P0)
- `maps/shell-flow.md` — how shell integration works end-to-end
- `maps/plugin-lifecycle.md` — the 4 plugin stages in detail
- `maps/event-flow.md` — event system from shell hook to Rust handler and back

## Universal Session Contract
Every session. No exceptions.
```bash
pt go                          # START — read handoff, check P0s
pt claim <issue>               # BEFORE touching any issue
pt done S-XXX success "..." "next action"   # END — after committing
```

## Task Router

| Task | Protocol |
|---|---|
| Implement a phase | GATE: PHASE below |
| Fix a bug | GATE: BUG FIX below |
| Add a crate or file | GATE: CRATE below + `maps/crate-deps.md` |
| Touch shell/ directory | GATE: SHELL below + `maps/shell-flow.md` |
| Implement or modify a plugin | GATE: PLUGIN below + `patterns/plugin-protocol.md` |
| Add or modify a theme | `patterns/testing.md` § Theme Tests |
| Add a pt issue | `patterns/phase-protocol.md` § Filing Issues |
| Add a pt decision | `pt check-decision "<keyword>"` first — never duplicate |
| Any work in crates/lynx-cli/ | Confirm command wired in commands/mod.rs dispatch |
| Any change to plugin lifecycle | `maps/plugin-lifecycle.md` FIRST |
| Any change to event system | `maps/event-flow.md` FIRST |

Gates are fill-in-the-blank audit trails. Copy, fill every field, emit before writing code. Blank field = violation.

---

## GATE: PHASE

```
G1 SCOPE
Phase:              B<N>-P<NN>
Decisions checked:  pt decisions <component> — relevant: D-XXX / none
Crate deps ok:      maps/crate-deps.md checked — no violations
Files planned:      (from phase do field)
AC understood:      (restate AC in your own words)
```
```
G2 IMPLEMENTATION
Files created/modified:   ____
Crate file limit check:   each file under 300 lines — yes / violations: ____
Dead code deleted:        none / list
Tests written:            crate unit tests: ____; bats tests: ____ / n/a
```
```
G3 VERIFICATION
Phase verify condition:         (copy from phase) — PASS / FAIL
AC check:                       each AC item — met / not met: ____
cargo nextest run -p <crate>:   PASS
scripts/verify-guardrails.sh:   PASS (run before every pt fix)
zsh -n on any emitted zsh:      PASS / n/a
Scope audit (if bug class):     rg "<pattern>" — 0 remaining / N filed as H-XXX
```
`pt fix B<N>-P<NN> "what was built" "how verified" "scope: grepped X, found N, all addressed"`

---

## GATE: BUG FIX

```
G1 SCOPE
Issue:              H-XXX
Decisions checked:  pt decisions <component> — relevant: D-XXX / none
Grep pattern:       rg "____" crates/ shell/ plugins/
Instances found:    N across M files
Files affected:     ____
All fixed here:     yes / no — if no: filed as H-XXX
```
```
G2 FIX
Root cause:         ____
Fix location(s):    file.rs:line / file.zsh:line
Tests added:        ____  (justify if impossible)
Dead code deleted:  none / list
```
```
G3 SCOPE AUDIT
Re-grep:                        rg "____"
Remaining instances:            0 / N — if N > 0: filed as H-XXX
cargo nextest run:              PASS
scripts/verify-guardrails.sh:   PASS
zsh -n (if shell):              PASS / n/a
```
`pt fix H-XXX "cause" "fix" "scope: grep 0 remaining in N files"`

---

## GATE: CRATE

```
G1 DESIGN
New crate name:         lynx-____
Purpose (one line):     ____
Allowed deps:           maps/crate-deps.md checked — upstream crates: ____
Forbidden deps:         would violate dep rules: ____
Is this a lib or bin?:  lib / bin — if bin: added to [[bin]] in Cargo.toml
Added to workspace:     Cargo.toml members[] — yes
```
```
G2 WIRING
Cargo.toml created:     yes — no unused workspace.dependencies
lib.rs has pub modules: ____
Used by:                crate(s) that depend on this: ____
Dead stub removed:      existing stub crate updated — yes / n/a
```

---

## GATE: SHELL

```
G1 THIN LAYER CHECK
File:               shell/____
Line count:         ____  (must be under 60 lines per file)
Contains logic?:    no — if yes: STOP. Move logic to Rust.
Calls lx binary?:   eval "$(lx ____)" pattern — yes / n/a
Silent on failure?: 2>/dev/null on all lx calls — yes
```
```
G2 VERIFY
zsh -n <file>:      PASS
Source in clean zsh subshell — no errors: PASS
```

---

## GATE: PLUGIN

Read `patterns/plugin-protocol.md` before writing any plugin code.

```
G1 MANIFEST
plugin.toml valid:      pt validate or lx plugin add ./plugins/<name> — PASS
exports explicit:       no wildcards, all names listed — yes
disabled_in set:        agent context listed if plugin exports aliases — yes / no aliases
binary deps declared:   deps.binaries lists all required binaries — yes
```
```
G2 SHELL GLUE
init.zsh under 10 lines:    yes / ____
functions use _ prefix for internals: yes
aliases in aliases.zsh only: yes
no git/external calls in render path: yes — uses segment cache
```
```
G3 CONTEXT CHECK
Tested in agent context:    aliases not loaded — yes
Tested in interactive:      full plugin loads — yes
```

---

## Hard Rules

1. **Crate dep direction (D-001):** Deps go down the tree only. `maps/crate-deps.md` is the law. Sideways deps = P0.
2. **TOML everywhere (D-003):** All config is TOML. Never write config as raw zsh or JSON.
3. **Shell layer is dumb (D-001):** No logic in shell/. Logic that creeps into shell/ must be moved to Rust immediately.
4. **Eval-bridge pattern only (D-001):** The shell evals `$(lx <cmd>)` output. Never source Rust output directly or pipe to bash.
5. **Agent context is automatic (D-004):** Never write code that requires users to manually set agent context. Detection is via env vars.
6. **Aliases are context-gated (D-010):** No plugin loads aliases unconditionally. Always disabled_in agent and minimal.
7. **Snapshot before mutate (D-007):** Every config mutation: snapshot → validate → apply. Non-negotiable.
8. **Secret redaction (B6-P06):** All lx output that may contain config values must pass through redact(). Use lynx-core::redact.
9. **File size limit:** 300 lines = warning. 500 lines = violation. Split the file.
10. **No dead code:** Every phase that adds code must delete what it replaces. grep to verify.
11. **Decisions before implementing:** `pt check-decision "<keyword>"` before implementing any non-trivial design. D-001 through D-010 are CORE.
12. **Runtime paths from runtime_dir() only (B3-P04):** Never hardcode /tmp or socket paths. Always use lynx-core::runtime::runtime_dir().

## Session End
```bash
cargo nextest run --all          # verify nothing broken
scripts/verify-guardrails.sh     # 38 invariant checks — must pass
pt done S-XXX success "what was done" "what next agent does first"
```
