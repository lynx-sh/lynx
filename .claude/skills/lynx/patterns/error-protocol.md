# Error Protocol (D-036)

**All user-facing errors use `LynxError::*`. Never `bail!()` for errors the user sees.**

```rust
use lynx_core::error::LynxError;
```

| Variant | Use for |
|---------|---------|
| `NotFound { item_type, name, hint }` | "X does not exist" |
| `AlreadyInstalled(name)` | Item already present |
| `NotInstalled(name)` | Required item absent |
| `Plugin(msg)` | Plugin load/validate/activate failure |
| `Theme(msg)` | Theme file invalid or missing |
| `Config(msg)` | Config file invalid |
| `Manifest(msg)` | plugin.toml parse failure |
| `Registry(msg)` | Registry fetch/parse failure |
| `Workflow(msg)` | Workflow schema or execution error |
| `Daemon(msg)` | Daemon service error |
| `Task(msg)` | Cron task scheduler error |
| `Shell(msg)` | Shell integration error |
| `Io { message, path, fix }` | IO with a known path |
| `io(err, path)` | IO from `std::io::Error` |

Adding a new variant: add to `LynxError` enum → add `hint()` arm → add `message()` arm → add test in `error.rs` → use in command code. Color/display logic stays in `error_display.rs` only.

**Verify after adding error paths:**
```bash
cargo nextest run -p lynx-core
cargo nextest run -p lynx-cli
```
