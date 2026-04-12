# Lynx Registry — Seed Catalog

This directory contains the official registry index used by `lx browse` and `lx install`.

The canonical copy lives at [lynx-sh/registry](https://github.com/lynx-sh/registry).
This local copy is the source of truth during development.

## Adding a package

1. Add an entry to `index.toml` following the existing format
2. Validate: `lx plugin index-validate registry/index.toml`
3. Sync to the registry repo

## Creating a community tap

See [docs/ecosystem.md](../docs/ecosystem.md) for the tap model documentation.
