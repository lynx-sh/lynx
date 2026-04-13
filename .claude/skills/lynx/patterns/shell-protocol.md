# Shell Protocol (D-001)

**No logic in shell/.** The zsh layer evals `lx` output only. Computation belongs in Rust.

| shell/ | Rust (crates/) |
|--------|----------------|
| `eval "$(lx ...)"` bootstrap | All init logic |
| Hook wiring (precmd/preexec/chpwd) | Hook handlers and output |
| PROMPT/RPROMPT assignment from lx | Prompt rendering |
| Nothing else | Everything else |

Any conditional logic, string manipulation, or control flow added to a `.zsh` file is a D-001 violation. Move it to Rust.

**Documented exception — eval error trap (eval-bridge.zsh):**
The `lynx_eval_safe` and `lynx_eval_plugin` functions in `shell/lib/eval-bridge.zsh`
capture `eval` stderr and check `$?`. This is I/O plumbing only — the shell layer never
formats or interprets the error. It delegates immediately to `lx shell-error <msg>` which
routes through `LynxError::Shell` in Rust. No new exceptions of this kind without a
matching D-001 exception note here.

**Verify:** `lx run lynx-ai-shell`
