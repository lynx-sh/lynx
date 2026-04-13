# Error Protocol — Lynx

All user-facing errors in `lx` commands must go through the centralized error system.
The renderer in `crates/lynx-cli/src/error_display.rs` formats them with color and hint lines.

## The rule

**Use `LynxError::*` variants for every user-facing error. Never use raw `bail!()` for errors the user will see.**

Raw `bail!()` still works (anyhow catches it), but it produces no hint line and no structured output. Reserve `bail!()` only for internal programmer errors that should never reach users.

## Available variants

All variants live in `lynx_core::error::LynxError`. Import with:
```rust
use lynx_core::error::LynxError;
```

| Variant | When to use | Example |
|---|---|---|
| `LynxError::NotFound { item_type, name, hint }` | Any "X does not exist" | Plugin not in registry, theme file missing, workflow not found |
| `LynxError::AlreadyInstalled(name)` | Item already present | `lx plugin add` on an installed plugin |
| `LynxError::NotInstalled(name)` | Required item is absent | `lx plugin reinstall` on a plugin that isn't installed |
| `LynxError::Plugin(msg)` | Plugin load/validate/activate failure | Manifest parse error, activation failure |
| `LynxError::Theme(msg)` | Theme file invalid or missing | TOML parse error, missing required field |
| `LynxError::Config(msg)` | Config file invalid | Schema validation failure |
| `LynxError::Manifest(msg)` | plugin.toml failed to parse | Invalid TOML, missing required field |
| `LynxError::Registry(msg)` | Registry fetch/parse failure | Network error, malformed index |
| `LynxError::Workflow(msg)` | Workflow schema or execution error | Missing steps, runtime failure |
| `LynxError::Daemon(msg)` | Daemon service error | Service not found, IPC failure |
| `LynxError::Task(msg)` | Cron task scheduler error | Invalid cron expression, task not found |
| `LynxError::Shell(msg)` | Shell integration error | Context detection failure, script generation |
| `LynxError::Io { message, path, fix }` | IO with a known path | File not found, permission denied |
| `LynxError::io(err, path)` | IO from `std::io::Error` | Use the constructor — auto-generates fix hint |

## Usage pattern

```rust
use anyhow::Result;
use lynx_core::error::LynxError;

// ❌ Don't do this — no hint line, no structured output
bail!("theme 'nord' not found");

// ✅ Do this — renderer shows the message + hint line
return Err(LynxError::NotFound {
    item_type: "Theme".into(),
    name: "nord".into(),
    hint: "run `lx theme list` to see available themes".into(),
}.into());

// ✅ Or this — shorter for simple cases
return Err(LynxError::Theme(format!("theme '{name}' is not valid: {e}")).into());
```

## Output the user sees

```
 error  Theme 'nord' not found
  hint  run `lx theme list` to see available themes
```

- ` error ` — bold white on red background
- `  hint ` — bold yellow
- hint text — dimmed

Color is suppressed when `NO_COLOR` is set or `TERM=dumb` or stdout is not a terminal.

## Adding a new variant

1. Add the variant to `crates/lynx-core/src/error.rs` `LynxError` enum
2. Add an arm to `hint()` returning the fix suggestion string
3. Add an arm to `message()` returning the primary message string
4. Add a test case in the `tests` module at the bottom of `error.rs`
5. Use `LynxError::YourVariant(...)` in the command code

**Do not add color, formatting, or display logic to `error.rs`** — that lives in `error_display.rs` (CLI layer only).

## Wiring mandate

After adding any new error path:
```bash
cargo nextest run -p lynx-core
cargo nextest run -p lynx-cli
```

Do not close a phase or issue that adds user-facing error paths without migrating them to `LynxError::*`.
