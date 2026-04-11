## Summary

<!-- What does this PR do? Why? Link the issue if applicable. Fixes #NNN -->

## Type of Change

- [ ] Bug fix (non-breaking)
- [ ] New feature (non-breaking)
- [ ] Breaking change (changes existing behavior)
- [ ] Documentation
- [ ] Refactor (no behavior change)

## Checklist

### All PRs
- [ ] `cargo nextest run --all` passes
- [ ] `cargo clippy --all -- -D warnings` is clean
- [ ] No new compiler warnings

### Bug fixes
- [ ] Regression test added (fails before fix, passes after)
- [ ] Grepped for the same bug pattern in other files — fixed everywhere

### New features
- [ ] Unit tests added
- [ ] Relevant docs updated (plugin guide / theme guide / README)
- [ ] `lx doctor` still passes on a clean install

### New plugin (first-party, in `plugins/`)
- [ ] `plugin.toml` has all required fields
- [ ] `[exports]` lists every symbol
- [ ] `[contexts].disabled_in` includes `"agent"` and `"minimal"` for aliases
- [ ] At least one bats test in `tests/integration/shell/`
- [ ] `lx doctor` shows no warnings

### Shell layer changes (`shell/`)
- [ ] Each modified file is under 60 lines
- [ ] No logic added — only thin glue and `lx` calls
- [ ] `zsh -n <file>` passes for each modified file

### Architecture changes
- [ ] ADR filed in `docs/decisions/` and linked from `docs/index.md`
- [ ] `maps/crate-deps.md` updated if dep graph changed

## Testing Notes

<!-- How did you test this? What edge cases did you consider? -->
