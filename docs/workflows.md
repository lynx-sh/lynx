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

## Workflow Schema

```toml
[workflow]
name = "deploy"
description = "Build, test, and deploy"
context = "interactive"     # only runs in interactive context (optional)

[workflow.params]
env = { type = "string", required = true, choices = ["staging", "production"] }
version = { type = "string", default = "latest" }

[[step]]
name = "build"
runner = "cargo"            # default: "sh"
run = "build --release"
group = "build"             # steps with same group run concurrently
env = { RUSTFLAGS = "-C target-cpu=native" }
cwd = "./backend"           # working directory for this step
on_fail = "abort"           # abort | retry | continue
timeout = 300               # seconds

[[step]]
name = "test"
run = "cargo nextest run"
depends_on = ["build"]      # wait for build to complete

[[step]]
name = "deploy"
run = "scripts/deploy.sh --env ${{params.env}}"
confirm = true              # ask before running
# confirm = "params.env == 'production'"  # conditional confirm
```

## Built-in Runners

| Runner | Binary | Example |
|--------|--------|---------|
| sh | sh -c | `run = "echo hello"` |
| zsh | zsh -c | `run = "print -P '%F{green}ok%f'"` |
| bash | bash -c | `run = "set -euo pipefail; ..."` |
| python | python3 | `run = "scripts/validate.py"` |
| node | node -e / npx | `run = "npm run build"` |
| go | go | `run = "run cmd/main.go"` |
| cargo | cargo | `run = "build --release"` |
| curl | curl | `run = "-X POST https://api.example.com"` |
| docker | docker | `run = "compose up -d"` |
| workflow | lx run | `run = "other-workflow"` (composability) |

Custom runners can be registered via plugin.toml `[runners.*]` tables.

## Execution Model

```
lx run deploy env=staging           # foreground, live output
lx run deploy env=staging --bg      # immediate background
lx run deploy --dry-run             # show steps without executing
lx run deploy env=production --yes  # skip confirms (CI mode)
```

During foreground execution:
- **Ctrl+B**: Migrate to background (output redirected to log file)
- **Ctrl+C**: Cancel workflow, run cleanup

## Job Management

```bash
lx jobs                    # list running and recent jobs
lx jobs view J-001         # live tail of a running job
lx jobs kill J-001         # cancel a running job
lx jobs log J-001          # view completed job's output
lx jobs clean              # remove old job logs
```

## Scheduled Workflows

Combine with `lx cron` to run workflows on a schedule:

```bash
lx cron add nightly-backup "0 3 * * *" "lx run backup"
```

## Distribution

Workflows are distributable via the registry tap system:

```bash
lx install devops-workflows    # installs workflow TOML files
lx run list                    # shows newly available workflows
lx audit devops-workflows      # shows every step before running
```
