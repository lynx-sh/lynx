# Troubleshooting Lynx

## Shell startup errors

### `(eval):N: unmatched '` or similar zsh eval errors

You may occasionally see a raw zsh error like this at terminal startup:

```
(eval):134: unmatched '
```

This means a piece of Lynx shell output contained invalid zsh syntax when your
shell tried to evaluate it. Lynx catches this and displays it as a styled error
message pointing you to the right fix command.

**What to do:**

```bash
lx doctor       # full health check with actionable fixes
lx diag         # view the raw diagnostic log for more detail
```

`lx doctor` is the primary self-healing entry point. It checks your config,
plugins, theme, and shell integration and tells you exactly what's wrong.

**Common causes:**

| Cause | Fix |
|-------|-----|
| Plugin manifest (`plugin.toml`) has invalid TOML | `lx plugin list` then fix or remove the bad plugin |
| Theme file is corrupted or missing | `lx theme list` then `lx theme set <name>` |
| Config file has invalid syntax | `lx doctor` will identify the exact field |
| Lynx binary is outdated | `lx update` |

If `lx doctor` reports all checks passing but the error persists, run
`lx diag` to see the full diagnostic log, then file an issue.

---

## Diagnostic tools

| Command | Purpose |
|---------|---------|
| `lx doctor` | Full health check — checks binary, config, plugins, theme, shell integration |
| `lx diag` | Raw diagnostic log — background errors logged during shell init |
| `lx diag clear` | Clear the diagnostic log |
| `lx diag path` | Show the path to the diagnostic log file |

---

## Tab completion not working

If `lx` tab completion stops working after a shell update or re-install:

```bash
lx init          # re-registers the completion file and fpath entry
exec zsh         # restart the shell to pick up the new completion
```

---

## Plugin not loading

```bash
lx doctor                        # check for plugin-specific failures
lx diag                          # see plugin load errors
lx plugin list                   # verify plugin is installed and enabled
lx plugin exec <name>            # test plugin eval output directly
```

---

## Config errors

```bash
lx doctor                        # identifies invalid config keys or values
lx config show                   # dump current config
lx rollback                      # restore previous config snapshot
```
