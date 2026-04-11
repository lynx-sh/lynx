# Crate Dependency Map

## Allowed Dependency Directions

```
lynx-core          ← NO internal deps. Foundation only.
├── lynx-config    ← lynx-core only
├── lynx-manifest  ← lynx-core only
├── lynx-events    ← lynx-core only
├── lynx-template  ← lynx-core, lynx-config
├── lynx-shell     ← lynx-core, lynx-manifest
│   └── lynx-loader    ← lynx-core, lynx-manifest, lynx-events, lynx-shell
│       └── lynx-prompt    ← lynx-core, lynx-config, lynx-events, lynx-template
│           └── lynx-theme     ← lynx-core, lynx-config
lynx-plugin        ← lynx-core, lynx-manifest, lynx-events, lynx-shell
lynx-task          ← lynx-core, lynx-config
lynx-daemon        ← lynx-core, lynx-events, lynx-task
lynx-registry      ← lynx-core, lynx-manifest
lynx-test-utils    ← ALL crates (dev-dependency only — never in [dependencies])
lynx-cli           ← ALL crates (assembles — never implements business logic)
```

## Forbidden Dependencies (P0 violations)

- lynx-core depending on ANYTHING internal
- lynx-prompt depending on lynx-loader (circular)
- lynx-events depending on lynx-plugin (circular)
- lynx-shell depending on lynx-cli
- Any crate depending on lynx-cli

## Sideways Dep Rule

Crates at the same level CANNOT import each other. They communicate through:
- lynx-events (event bus) for runtime communication
- lynx-core types for shared data structures
- lynx-cli as the only assembler

## When Adding a New Crate

1. Identify where it sits in the tree above
2. List ONLY its allowed upstreams
3. If it needs something sideways — use the event bus instead
4. Update this map
