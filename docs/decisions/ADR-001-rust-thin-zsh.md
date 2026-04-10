# ADR-001: Rust Core with Thin Zsh Integration Layer

**Status:** Accepted

## Context
Shell frameworks traditionally written in pure zsh are slow, hard to test, and difficult to maintain at scale. Startup times of 500ms–2s are common in OMZ. Logic in shell script is untyped, untestable, and brittle.

## Decision
All framework logic lives in Rust crates. The zsh layer is intentionally kept thin (~200 lines total) and dumb — it only evals output from the `lx` binary. Every Rust tool that integrates with shells (Starship, zoxide, atuin) uses the same eval pattern successfully.

## Consequences
- Startup cost: ~1-3ms for Rust binary invocation. Acceptable.
- Shell integration requires a build step (Cargo). Installer handles this.
- Zsh logic becomes trivially testable as Rust unit tests.
- Portability requires a binary per architecture — handled by install.sh.
