# Phase Protocol

## Before Scaffolding
1. Verify every file listed exists — mark non-existent files as `wiring_required`
2. `pt decisions <component>` for each affected component — reflect in `do`/`dont`
3. Read the crate before designing phases — don't design "add X" if X exists

## Commands
```bash
pt block-add B<N> "Title" "Acceptance criteria"
pt scaffold-phases B<N> < phases.json   # top-level array of phase objects
pt phases B<N>                          # verify after — read each phase cold
```

## Phase Field Requirements
- **`do`**: name the exact file/function, state why, state what to delete if replacing
- **`dont`**: the most dangerous wrong approach + reason (decision ID or incident)
- **`verify`**: at least one negative case + `cargo nextest run -p lynx-<crate>`
- **`files`**: only files actually modified — new files require `wiring_required`

## Issues Found During Phases
File with `pt add` immediately. If blocking: claim and fix inline. Never silently fix.
