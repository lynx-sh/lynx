# Shell Protocol (D-001)

**No logic in shell/.** The zsh layer evals `lx` output only. Computation belongs in Rust.

| shell/ | Rust (crates/) |
|--------|----------------|
| `eval "$(lx ...)"` bootstrap | All init logic |
| Hook wiring (precmd/preexec/chpwd) | Hook handlers and output |
| PROMPT/RPROMPT assignment from lx | Prompt rendering |
| Nothing else | Everything else |

Any conditional logic, string manipulation, or control flow added to a `.zsh` file is a D-001 violation. Move it to Rust.

**Verify:** `lx run lynx-ai-shell`
