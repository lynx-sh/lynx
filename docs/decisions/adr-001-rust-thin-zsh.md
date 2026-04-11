# ADR-001: Rust Core with Thin zsh Eval-Bridge

**Status:** Accepted

## Context

Shell frameworks are traditionally written in shell script (zsh/bash). This
keeps distribution simple but creates deep problems at scale: no type safety,
no unit tests, performance degrades with complexity, and debugging is painful.

The initial Lynx prototype was ~800 lines of pure zsh. Adding features (async
task scheduler, plugin isolation, context detection) was increasingly risky.
Every new feature made the framework slower and harder to test.

The alternative — writing everything in a compiled language with a thin shell
glue layer — had been proven viable by Starship (prompt-only, Rust binary).

## Decision

All business logic lives in Rust crates. The zsh layer is a thin (~200 line)
eval-bridge that does nothing except:

1. Source the Lynx init file on shell startup
2. Eval output from `lx` subcommands (the eval-bridge pattern)
3. Forward zsh hooks (`chpwd`, `precmd`, `preexec`) as events to `lx`

The `lx` binary prints zsh to stdout. The shell evals it. No Rust code ever
sources files directly — `eval "$(lx <cmd>)"` is the only integration point.

**The shell layer has a hard file-size limit**: 60 lines per file, 0 lines of
logic. Any conditional or loop in shell/ is a violation that must be moved
to Rust.

## Consequences

**Positive:**
- All logic is unit-tested in Rust with `cargo nextest`
- Shell layer is small enough to read and audit in 5 minutes
- New features are added in Rust without touching shell/
- Startup time is predictable — one `lx init` call, then eval

**Negative:**
- Contributors need Rust toolchain (not just zsh knowledge)
- `lx` binary must be compiled and on `$PATH` before the shell can init
- Eval-bridge adds one process spawn per init (mitigated: it's one call)

**Invariants this creates:**
- D-001: No logic in shell/ — enforced in PR review
- D-001: Eval-bridge pattern only — `eval "$(lx ...)"` everywhere
