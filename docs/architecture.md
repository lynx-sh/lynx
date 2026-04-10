# Lynx Architecture

## Crate Dependency Graph

```
lynx-core          (no internal deps — foundation types only)
├── lynx-config    (user config TOML)
├── lynx-manifest  (plugin.toml parser)
├── lynx-events    (event bus)
├── lynx-template  (token substitution)
└── lynx-shell     (zsh glue generator)
    └── lynx-loader    (dep graph, plugin lifecycle)
        └── lynx-prompt    (segment evaluation, rendering)
            └── lynx-theme     (theme TOML loader)
lynx-task          (task scheduler, cron)
lynx-daemon        (background process, IPC socket)
lynx-registry      (plugin index, fetch, version)
lynx-cli           (lx binary — assembles all crates, never implements)
```

## Shell Integration Flow

```
.zshrc
  └── source ~/.config/lynx/shell/init.zsh
        └── source shell/core/loader.zsh
              └── eval "$(lx init --context <detected>)"
                    ├── sets LYNX_DIR, LYNX_CONTEXT, LYNX_PLUGIN_DIR
                    ├── sources shell/core/hooks.zsh
                    └── for each enabled plugin:
                          eval "$(lx plugin exec <name>)"
```

## Plugin Lifecycle

```
DECLARE → RESOLVE → LOAD → ACTIVATE

DECLARE:  parse all plugin.toml manifests from plugins/
RESOLVE:  topological sort by deps, apply context filter
LOAD:     eager plugins: exec now; lazy: register deferred trigger
ACTIVATE: register event subscriptions from manifest hooks[]
```

## Event System

```
zsh hook (chpwd/preexec/precmd)
  └── _lynx_hook_* function
        └── lx event emit "shell:chpwd" --data "$PWD"
              └── IPC → Unix socket → lynx-daemon
                    └── EventBus.dispatch()
                          └── registered plugin handlers
```

## Data Flow: Config Mutation

```
lx <mutating command>
  ├── snapshot current config
  ├── validate new state
  ├── apply to disk
  └── emit config:changed event
        └── shell reloads affected components
```
