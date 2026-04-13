# Lynx Workflows

## Overview

Workflows are TOML-defined multi-step pipelines executed by `lx run`.
Each step declares a runner (sh, python, cargo, etc.) and Lynx orchestrates
execution order, concurrency, signals, and logging.

## Related Decisions

Run `pt decisions arch` and `pt decisions cli` for workflow-related decisions.

## Quick Start

```bash
# Create a workflow
mkdir -p ~/.config/lynx/workflows
cat > ~/.config/lynx/workflows/build.toml << 'EOF'
[workflow]
name = "build"
description = "Build and test the project"

[[step]]
name = "build"
run = "cargo build --release"

[[step]]
name = "test"
run = "cargo nextest run --all"
EOF

# Run it
lx run build

# List available workflows
lx run list
```

---

## Workflow TOML Schema

### `[workflow]` — Metadata

| Field         | Type     | Required | Default | Description                                       |
|---------------|----------|----------|---------|---------------------------------------------------|
| `name`        | string   | yes      | —       | Unique workflow name                              |
| `description` | string   | no       | `""`    | Human-readable description shown in `lx run list` |
| `context`     | string   | no       | any     | Restrict to a context: `interactive`, `agent`, or `minimal` |

### `[[workflow.param]]` — Parameters

Workflows accept typed parameters passed as `key=value` on the command line.
Inside `run` strings, reference them as `$param_name`.

| Field         | Type     | Required | Default  | Description                                          |
|---------------|----------|----------|----------|------------------------------------------------------|
| `name`        | string   | yes      | —        | Parameter name                                       |
| `type`        | string   | no       | `string` | One of: `string`, `int`, `bool`                      |
| `required`    | bool     | no       | `true`   | Whether the caller must supply this parameter         |
| `default`     | string   | no       | —        | Value used when the parameter is not provided         |
| `choices`     | string[] | no       | any      | Allowed values; Lynx rejects anything outside the list |
| `description` | string   | no       | `""`     | Help text shown in error messages and prompts         |

```toml
[workflow]
name = "deploy"

[[workflow.param]]
name = "env"
type = "string"
required = true
choices = ["staging", "production"]
description = "Target environment"

[[workflow.param]]
name = "dry_run"
type = "bool"
required = false
default = "false"
```

```bash
lx run deploy env=staging
lx run deploy env=production dry_run=true
```

### `[[step]]` — Steps

| Field         | Type     | Required | Default  | Description                                                      |
|---------------|----------|----------|----------|------------------------------------------------------------------|
| `name`        | string   | yes      | —        | Unique step name within the workflow                             |
| `run`         | string   | yes      | —        | Command or script to execute; `$param_name` is substituted       |
| `runner`      | string   | no       | `sh`     | Runner to use — see [Runners](#runners) below                    |
| `confirm`     | bool     | no       | `false`  | Prompt for confirmation before executing this step               |
| `timeout_sec` | integer  | no       | none     | Kill the step after N seconds                                    |
| `on_fail`     | string   | no       | `abort`  | `abort` \| `retry` \| `continue`                                |
| `retry_count` | integer  | no       | `0`      | Retry attempts when `on_fail = "retry"`                          |
| `condition`   | string   | no       | —        | Skip step unless truthy: `"$param == value"` or `"env:VAR_NAME"` |
| `depends_on`  | string[] | no       | `[]`     | Steps that must complete before this one starts                  |
| `group`       | string   | no       | —        | Steps sharing a group name run concurrently                      |
| `env`         | table    | no       | `{}`     | Extra environment variables scoped to this step                  |
| `cwd`         | string   | no       | current  | Working directory for this step                                  |

---

## Runners

| Runner   | Executes via       | Notes                                    |
|----------|--------------------|------------------------------------------|
| `sh`     | `sh -c`            | Default runner                           |
| `bash`   | `bash -c`          | Use for `set -euo pipefail` scripts      |
| `zsh`    | `zsh -c`           | Use for zsh-specific syntax              |
| `python` | `python3`          | Runs a file path or inline script        |
| `node`   | `node -e` / `npx`  | JS/TS scripts                            |
| `go`     | `go`               | Prepends `go` to the run string          |
| `cargo`  | `cargo`            | Prepends `cargo` to the run string       |
| `curl`   | `curl`             | Prepends `curl` to the run string        |
| `docker` | `docker`           | Prepends `docker` to the run string      |
| custom   | plugin-defined     | Register via `[runners.*]` in plugin.toml |

---

## Concurrency and Ordering

Steps run sequentially by default. Two mechanisms change that:

**`group`** — steps sharing a group name run in parallel:

```toml
[[step]]
name = "lint"
run = "cargo clippy --all"
group = "checks"

[[step]]
name = "test"
run = "cargo nextest run --all"
group = "checks"          # runs at the same time as lint

[[step]]
name = "report"
run = "scripts/report.sh"
depends_on = ["lint", "test"]   # waits for both
```

**`depends_on`** — explicit ordering across groups. Lynx validates that every
referenced step name exists at parse time.

---

## Failure Handling

| `on_fail`  | Behaviour                                              |
|------------|--------------------------------------------------------|
| `abort`    | Stop the workflow immediately (default)                |
| `retry`    | Retry the step up to `retry_count` times, then abort   |
| `continue` | Log the failure and proceed to the next step           |

```toml
[[step]]
name = "flaky-check"
run = "curl -f https://api.example.com/health"
on_fail = "retry"
retry_count = 3
timeout_sec = 10
```

---

## lx run

```bash
lx run <name>                       # run a workflow (foreground, live TUI)
lx run <name> key=val key2=val2     # pass parameters
lx run <name> --dry-run             # print steps without executing
lx run <name> --bg                  # run immediately in background
lx run <name> --yes                 # skip all confirmation prompts (CI mode)
lx run list                         # browse available workflows (TUI)
lx run examples                     # show full TOML examples
```

**Flags**

| Flag        | Description                                             |
|-------------|---------------------------------------------------------|
| `--dry-run` | Print each step's resolved run string; do not execute   |
| `--bg`      | Send to background immediately; tail with `lx jobs log` |
| `--yes`     | Auto-confirm all `confirm = true` steps                 |

**During foreground execution**

- **Ctrl+B** — migrate to background (output redirected to log file)
- **Ctrl+C** — cancel workflow and run cleanup

Workflow files live in `~/.config/lynx/workflows/`.

---

## lx jobs

Manage workflow job history and running jobs.

| Subcommand                  | Description                                         |
|-----------------------------|-----------------------------------------------------|
| `lx jobs list`              | Browse recent jobs (TUI) with status and duration   |
| `lx jobs view <id>`         | Print full JSON record for a job                    |
| `lx jobs kill <id>`         | Send kill signal to a running job                   |
| `lx jobs log <id>`          | Print captured output for a job                     |
| `lx jobs clean`             | Remove job records older than 72 hours (default)    |
| `lx jobs clean --hours <N>` | Remove records older than N hours                   |

---

## Scheduled Workflows

Combine with `lx cron` to run workflows on a schedule:

```bash
lx cron add nightly-backup --run "lx run backup" --cron "0 3 * * *"
lx cron list                        # see all scheduled tasks and last-run status
lx cron logs nightly-backup         # view cron execution logs
```

---

## Distribution

Workflows are distributable via the registry tap system:

```bash
lx install devops-workflows    # installs workflow TOML files to workflows/
lx run list                    # shows newly available workflows
lx audit devops-workflows      # shows every step before running
```

---

## Worked Example

A complete multi-step release workflow with parameters, concurrency, and failure handling:

```toml
[workflow]
name = "release"
description = "Build, test, and publish a release"
context = "interactive"

[[workflow.param]]
name = "version"
type = "string"
required = true
description = "Release version (e.g. 1.2.3)"

[[workflow.param]]
name = "env"
type = "string"
required = false
default = "staging"
choices = ["staging", "production"]

# --- parallel checks ---

[[step]]
name = "lint"
run = "cargo clippy --all -- -D warnings"
group = "checks"

[[step]]
name = "test"
run = "cargo nextest run --all"
group = "checks"

# --- build (waits for checks) ---

[[step]]
name = "build"
runner = "cargo"
run = "build --release"
depends_on = ["lint", "test"]
timeout_sec = 300

# --- publish (confirm before running) ---

[[step]]
name = "publish"
runner = "bash"
run = "scripts/publish.sh --version $version --env $env"
depends_on = ["build"]
confirm = true
on_fail = "retry"
retry_count = 2
[step.env]
RELEASE_TOKEN = "${RELEASE_TOKEN}"
```

```bash
lx run release version=1.2.3
lx run release version=1.2.3 env=production
lx run release version=1.2.3 --dry-run   # preview steps
lx run release version=1.2.3 --yes       # skip confirm prompt (CI)
```
