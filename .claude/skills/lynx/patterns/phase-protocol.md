# Phase and Issue Protocol

## Working a Phase

```bash
pt go                         # always first
pt claim B<N>-P<NN>           # before touching any file
# ... implement ...
pt phase-done B<N>-P<NN> "outcome summary"
git commit ...
pt done S-XXX success "..." "next agent: ..."
```

## Phase Verification Requirement

Before marking a phase done, you MUST verify the AC conditions listed in the phase.
Run `pt phase B<N>-P<NN>` to read the AC field. Every AC item must be checked off.

If an AC item cannot be verified (e.g. requires a binary not yet built), document why
and file an H-XXX issue for it.

## Filing Issues During Phase Work

If you discover a problem that's out of scope for the current phase:
```bash
pt add "title" P<level> <component> "problem description" "fix_required: In <file/fn>: what to do"
```

Do NOT silently note and move on. Do NOT fix out-of-scope bugs in the current phase commit.
File the issue and continue with the phase.

## Scope Check Before Fixing Bugs

Before fixing any bug found during phase work:
```bash
pt scope-check H-XXX   # or grep manually
rg "<pattern>" crates/ shell/ plugins/
```

Fix ALL instances in one commit. If there are >3 files affected, this may be a separate phase.

## Adding New Phases

If you discover work that isn't covered by existing phases:
1. Check if it fits inside an existing block
2. If yes: add it with `pt scaffold-phases B<N> < phases.json` (single-phase array)
3. If no: propose a new block to the architect (don't create blocks without review)

## Component Labels

Use these for `pt add`:
- `core` — lynx-core, lynx-config, lynx-manifest
- `shell` — shell/ directory, eval-bridge, hooks
- `plugins` — plugin system, individual plugins
- `prompt` — lynx-prompt, lynx-theme, segments
- `events` — lynx-events, event bus, IPC
- `cli` — lynx-cli, lx commands
- `task` — lynx-task, lynx-daemon, scheduler
- `registry` — lynx-registry, plugin index
- `dx` — install.sh, doctor, benchmark, onboarding
- `docs` — docs/, README, CONTRIBUTING
- `ci` — GitHub Actions, release pipeline
