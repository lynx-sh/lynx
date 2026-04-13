# Shell Protocol — Lynx

## The Rule (D-001)
**No logic in `shell/`.** The zsh layer is a thin eval-bridge (~200 lines). It evals output from `lx`. That's it.

## What Goes Where

| Belongs in `shell/` | Belongs in Rust (`crates/`) |
|---|---|
| `eval "$(lx init)"` bootstrap | All init logic |
| Hook wiring (`precmd`, `preexec`, `chpwd`) | Hook handlers and output |
| PROMPT/RPROMPT assignment from lx output | Prompt rendering |
| Nothing else | Everything else |

## Before Touching `shell/`

```bash
pt decisions shell
pt decisions arch
```

If your change adds conditional logic, string manipulation, array operations, or any control flow to a `.zsh` file — **stop**. Move that logic to Rust and have the shell eval the output.

## Verification

```bash
bats tests/integration/shell/
cargo nextest run -p lynx-shell
```
