# Contributing to Lynx

Thank you for contributing. This document covers the dev environment, how to
run tests, the commit convention, and the PR process.

The bar for a bug fix is low. The bar for a new plugin or architectural change
is higher. This document explains what each type of change needs.

---

## Table of Contents

1. [Dev Environment](#dev-environment)
2. [Running Tests](#running-tests)
3. [Commit Convention](#commit-convention)
4. [PR Process](#pr-process)
5. [Filing a Bug](#filing-a-bug)
6. [Plugin Submission](#plugin-submission)
7. [Theme Submission](#theme-submission)
8. [Architecture Decision Records (ADRs)](#architecture-decision-records-adrs)

---

## Dev Environment

**Requirements:**
- Rust toolchain (stable): `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- `cargo-nextest`: `cargo install cargo-nextest`
- `bats-core` (for shell integration tests): `brew install bats-core` (macOS)
- `zsh` (for shell tests): pre-installed on macOS, `apt install zsh` on Linux

**Setup:**

```bash
git clone https://github.com/proxikal/lynx.git
cd lynx
cargo build
```

**Build the lx binary and add to PATH for shell testing:**

```bash
cargo build --release
export PATH="$PWD/target/release:$PATH"
lx --version
```

---

## Running Tests

**All Rust tests:**
```bash
cargo nextest run --all
```

**Single crate:**
```bash
cargo nextest run -p lynx-plugin
cargo nextest run -p lynx-prompt
```

**Shell integration tests (bats):**
```bash
bats tests/integration/shell/
```

**Before submitting a PR:**
```bash
cargo nextest run --all                  # must pass
bats tests/integration/shell/           # must pass
cargo clippy --all -- -D warnings        # must be clean
```

---

## Commit Convention

```
type(scope): short description

Body (optional) — explain the why, not the what.
```

Types: `feat`, `fix`, `refactor`, `test`, `docs`, `chore`

Scope is the crate or component: `lynx-plugin`, `lynx-prompt`, `shell`, `docs`

Examples:
```
feat(lynx-plugin): add hook wiring to exec script generator
fix(lynx-cli): plugin unload command missing from dispatch
docs(plugin-authoring): add worked weather plugin example
```

- Keep the subject line under 72 characters
- Use the imperative mood ("add", "fix", "remove" — not "adds", "fixed")
- Reference issues in the body: `Fixes #123`

---

## PR Process

### Bug fixes

1. Open an issue (or reference an existing one)
2. Fix the bug — make sure you fix the entire class, not just one instance
3. Add a regression test
4. Submit PR with the issue number in the description

A bug fix PR needs: a test that fails before the fix and passes after. It does
not need docs updates unless the fix changes public behavior.

### New features

1. Open an issue describing the feature — include the motivation
2. Wait for a maintainer to comment before investing significant effort
3. Implement, with tests
4. Update relevant docs (plugin guide, theme guide, README if visible)
5. Submit PR

### Architectural changes

Architectural changes need an ADR. See [ADRs](#architecture-decision-records-adrs).
Do not implement a significant design change without an accepted ADR.

---

## Filing a Bug

Use the [Bug Report template](.github/ISSUE_TEMPLATE/bug.md). Include:

- `lx --version` output
- `lx doctor` output
- Steps to reproduce
- Expected vs actual behavior
- Your `~/.config/lynx/config.toml` (redact any secrets)

---

## Plugin Submission

To add a plugin to the official registry:

1. Build and test your plugin locally (see [Plugin Authoring Guide](docs/plugin-authoring.md))
2. Publish to a public git repo
3. Ensure `lx plugin add ./your-plugin` + `lx doctor` pass cleanly
4. Open a PR to [lynx-plugins](https://github.com/proxikal/lynx-plugins) adding
   your entry to `index.toml`

**Quality bar for accepted registry plugins:**

- [ ] `plugin.toml` has all required fields
- [ ] `[exports]` lists every symbol
- [ ] `[deps].binaries` lists every required binary
- [ ] `[contexts].disabled_in` includes `"agent"` and `"minimal"` for aliases
- [ ] At least one bats test covering the main function
- [ ] Plugin loads cleanly on a fresh install: `lx doctor` shows no warnings
- [ ] Doesn't shadow common command names (check: `which <your_alias>`)

Use the [Plugin Request template](.github/ISSUE_TEMPLATE/plugin-request.md) if
you want to request a plugin without building it yourself.

---

## Theme Submission

To add a theme to the official themes directory:

1. Create `themes/<name>.toml` following the [Theme Authoring Guide](docs/theme-authoring.md)
2. Test it: `lx theme switch <name>` then open a new terminal
3. Run `lx doctor` — should show no unknown segments
4. Submit a PR with the theme file

**Quality bar:**
- [ ] `[meta].name` matches the filename (without `.toml`)
- [ ] Only uses segments that are in `KNOWN_SEGMENTS` (no forward references)
- [ ] Tested in interactive and agent context
- [ ] Colors degrade gracefully (test in a 256-color terminal)

---

## Architecture Decision Records (ADRs)

Significant architectural decisions are documented as ADRs in `docs/decisions/`.

**When to write an ADR:**
- Adding a new crate
- Changing the dependency direction between crates
- Changing the plugin lifecycle
- Changing the config schema in a breaking way
- Adding a new context type

**Format:**

```markdown
# ADR-NNN: Short Title

**Status:** Proposed | Accepted | Superseded by ADR-NNN

## Context
Why is this decision being made? What problem does it solve?

## Decision
What was decided, precisely.

## Consequences
What becomes easier? What becomes harder? What invariants does this create?
```

File your ADR as `docs/decisions/adr-NNN-short-title.md` and add it to the
[index](docs/index.md). Open a PR for discussion before marking it Accepted.
