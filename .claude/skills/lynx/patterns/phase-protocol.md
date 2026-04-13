# Block & Phase Protocol — Lynx

## Before Scaffolding Any Block

1. **Verify every file you list exists:** `ls path/to/file.rs` — if it doesn't exist, mark it as `produces` with `wiring_required`
2. **Run `pt decisions <component>`** for each affected component — reflect constraints in phase `do`/`dont` fields
3. **Check for existing infrastructure** before designing phases to "add X" — read the crate first

## Scaffolding Commands

```bash
pt block-add B<N> "Title" "Acceptance criteria"
pt scaffold-phases B<N> < phases.json    # top-level array of phase objects

# After — mandatory self-review
pt phases B<N>                           # verify all phases
pt phase B<N>-P01                        # read each cold — would a new agent know what to do?
```

## Phase Field Requirements

### `do` field (min 40 chars)
- Name the specific function/file/location
- State WHY, not just WHAT
- If replacing code: state what to DELETE and require a grep to confirm

### `dont` field
- List the most dangerous wrong approaches
- Reference the reason (decision ID, past incident)

### `verify` field
- At least 2 specific test scenarios
- At least one negative case
- Include `cargo nextest run -p lynx-<crate>` with the relevant crate

### `files` field
- Only files that will actually be modified
- New files require `wiring_required`

## Responsibility Check (D-042) — Required for Every Phase

Every phase that creates or significantly modifies files must include:
```
Responsibility check:
  <file>: does ____ (one domain) — ok
  <file>: does ____ and ____ — SPLIT REQUIRED before proceeding
```

## Issues Found During Phases

1. Do NOT silently fix bugs found during phase work
2. `pt add` immediately — file the issue
3. If blocking: `pt claim` and fix inline
4. If not blocking: file and leave for next agent
