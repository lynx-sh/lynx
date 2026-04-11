# Lynx Plugin Registry — Index Specification

The Lynx plugin registry is a **static TOML file** hosted in a git repository.
There is no registry server — the client fetches the raw file directly from GitHub.

Official index: `https://raw.githubusercontent.com/proxikal/lynx-plugins/main/index.toml`

---

## Index Schema

The index file contains one or more `[[plugin]]` tables, each describing a single plugin.

```toml
[[plugin]]
name            = "git"                          # required — must match plugin.toml [plugin].name
description     = "Git integration for Lynx"    # required — shown in lx plugin search
author          = "proxikal"                     # required
latest_version  = "1.2.0"                       # required — must match one entry in versions[]

[[plugin.versions]]
version          = "1.2.0"                      # semver — required
url              = "https://github.com/proxikal/lynx-plugins/releases/download/git-1.2.0/git-1.2.0.tar.gz"
checksum_sha256  = "e3b0c44298fc1c149afb..."    # SHA-256 hex digest — required, no exceptions
min_lynx_version = "0.1.0"                      # optional — minimum Lynx version required

[[plugin.versions]]
version         = "1.1.0"
url             = "https://..."
checksum_sha256 = "abc123..."
```

### Required fields

| Field | Description |
|---|---|
| `name` | Unique plugin identifier. Must match `[plugin].name` in `plugin.toml`. |
| `description` | One-line description shown in `lx plugin search`. |
| `author` | Maintainer name or GitHub handle. |
| `latest_version` | The version string of the recommended release. Must be present in `versions[]`. |
| `versions[].version` | Semver string (`MAJOR.MINOR.PATCH`). |
| `versions[].url` | Direct download URL for the `.tar.gz` archive. |
| `versions[].checksum_sha256` | SHA-256 hex digest of the archive. **Cannot be empty.** |

### Optional fields

| Field | Default | Description |
|---|---|---|
| `versions[].min_lynx_version` | `null` | Minimum Lynx version required to load this plugin. |

---

## Archive Format

The release archive must be a `.tar.gz` with a single top-level directory:

```
git-1.2.0/
├── plugin.toml          ← required
└── shell/
    ├── init.zsh
    ├── functions.zsh
    └── aliases.zsh
```

The installer strips the top-level component — contents land directly in
`~/.local/share/lynx/plugins/<name>/`.

---

## Generating the Checksum

Use the `sha256sum` command (Linux) or `shasum -a 256` (macOS):

```bash
# macOS
shasum -a 256 git-1.2.0.tar.gz

# Linux
sha256sum git-1.2.0.tar.gz

# Using lx (built-in helper — same algorithm as verification):
lx plugin checksum ./git-1.2.0.tar.gz
```

Copy the hex digest (64 characters) into `checksum_sha256`. The installer rejects
archives where the digest does not match exactly.

---

## Submission Process

1. **Fork** `https://github.com/proxikal/lynx-plugins`
2. **Create a release** on your plugin's repository and upload the `.tar.gz` archive
3. **Generate the checksum** with `shasum -a 256 <archive>.tar.gz`
4. **Add an entry** to `index.toml` following the schema above
5. **Open a PR** — the review checklist is in the PR template

### Automated validation

The PR CI runs:

```bash
# Validate index TOML parses correctly
lx plugin index-validate index.toml

# For each new/updated plugin entry:
# - Downloads the archive
# - Verifies the checksum
# - Runs lx doctor on the extracted plugin
```

---

## Quality Requirements

Every plugin submitted to the official index must pass these checks.
They mirror what `lx doctor` validates on a clean install.

| Requirement | Check |
|---|---|
| `plugin.toml` is valid TOML | `lx plugin add <path>` succeeds |
| `[plugin].name` matches directory name | Verified at extraction |
| All exported functions listed explicitly | `exports.functions` — no wildcards |
| All exported aliases listed explicitly | `exports.aliases` — no wildcards |
| Aliases are context-gated | `disabled_in = ["agent", "minimal"]` required if `aliases` is non-empty |
| No logic in `shell/init.zsh` | `init.zsh` must be ≤ 10 lines; no if/for/while |
| Binary deps declared | All external tools listed in `deps.binaries` |
| Shell files pass `zsh -n` | Syntax check on all `.zsh` files |
| No hardcoded paths | No `/tmp`, `/home`, or `/Users` in shell files |
| File size limit respected | No file over 500 lines |

---

## Versioning Conventions

- Use **semver** (`MAJOR.MINOR.PATCH`).
- Bump `PATCH` for bug fixes.
- Bump `MINOR` for new functions or aliases (backwards compatible).
- Bump `MAJOR` for breaking changes (renamed/removed exports).
- Keep **all previous versions** in the index — users may pin to an older version.
- Set `min_lynx_version` when your plugin uses a Lynx API introduced in a specific release.

---

## lynx.lock

When `lx plugin add <name>` fetches from the registry, it writes an entry to
`~/.config/lynx/lynx.lock`:

```toml
[[locked]]
name             = "git"
version          = "1.2.0"
checksum_sha256  = "e3b0c44298fc1c149afb..."
url              = "https://github.com/..."
source           = "registry"
```

`lynx.lock` is included in git sync (`lx sync push`) so installs are reproducible
across machines. Locally-installed plugins (added via path) have `source = "local"`
and are not updated by `lx plugin update --all`.
