# Lynx Dashboard

## Overview

`lx dashboard` starts a local web server that opens a full management UI in your browser.
Every aspect of Lynx is configurable from this single interface — themes, plugins, registry,
workflows, intros, and system diagnostics.

## Related Decisions

Run `pt decisions cli` and `pt decisions arch` for dashboard-related decisions.

## Usage

```bash
lx dashboard              # starts server, opens browser
lx dashboard --port 8080  # use specific port (default: random)
# Ctrl+C to stop
```

## Architecture

```
lx dashboard
    │
    ▼
┌──────────────────────────────────────────────────────┐
│  Axum HTTP Server (localhost, random port)            │
├──────────────────────────────────────────────────────┤
│                                                       │
│  Static Assets (embedded via include_str!)            │
│  GET /              → index.html                     │
│  GET /css/*.css     → stylesheets                    │
│  GET /js/*.js       → JavaScript modules             │
│                                                       │
│  API Endpoints                                        │
│  GET  /api/config          → current config           │
│  POST /api/config/patch    → mutate config (D-007)    │
│  GET  /api/themes          → list themes              │
│  POST /api/theme/set       → switch theme             │
│  POST /api/theme/patch     → WYSIWYG mutation         │
│  GET  /api/plugins         → list with manifests      │
│  POST /api/plugin/enable   → toggle on                │
│  POST /api/plugin/disable  → toggle off               │
│  GET  /api/registry/browse → merged tap index         │
│  POST /api/registry/install → install package         │
│  GET  /api/taps            → list taps                │
│  POST /api/tap/add         → add community tap        │
│  GET  /api/workflows       → list workflows           │
│  POST /api/workflow/run    → start workflow            │
│  GET  /api/jobs            → running/recent jobs       │
│  GET  /api/job/:id/stream  → SSE live output           │
│  GET  /api/cron            → scheduled tasks           │
│  GET  /api/intros          → available intros          │
│  GET  /api/doctor          → diagnostic results        │
│  GET  /api/diag            → recent diag log           │
│  GET  /events              → SSE broadcast (all state)  │
│                                                       │
│  All mutations use existing library functions.         │
│  Dashboard implements NO business logic.               │
│                                                       │
└──────────────────────────────────────────────────────┘
    ▲                           │
    │ fetch() / POST            │ SSE (Server-Sent Events)
    │                           ▼
┌──────────────────────────────────────────────────────┐
│  Browser (system default)                             │
├──────────────────────────────────────────────────────┤
│                                                       │
│  Sidebar Navigation                                   │
│  ├── Overview      status cards, at-a-glance          │
│  ├── Themes        WYSIWYG editor (ported from studio)│
│  ├── Plugins       toggle switches, quick install     │
│  ├── Registry      visual package browser             │
│  ├── Workflows     run workflows, view job output     │
│  ├── Cron          scheduled task management          │
│  ├── Intros        preview and switch intros           │
│  └── System        config editor, doctor, diag log    │
│                                                       │
│  Client-side hash routing (#/overview, #/themes, etc) │
│  SSE listener updates all pages in real-time           │
│  Global state store fed by SSE events                  │
│                                                       │
└──────────────────────────────────────────────────────┘
```

## Frontend File Structure

All files are embedded in the binary. No npm, no build step.

```
crates/lynx-dashboard/src/frontend/
├── mod.rs              # include_str! + serve routes
├── index.html          # shell layout, sidebar nav
├── css/
│   ├── base.css        # reset, CSS custom properties (design tokens)
│   ├── layout.css      # sidebar, grid, responsive
│   ├── components.css  # cards, toggles, buttons, inputs
│   └── pages.css       # page-specific styles
└── js/
    ├── app.js          # router, SSE, state store
    ├── api.js          # fetch wrappers for all endpoints
    ├── pages/
    │   ├── overview.js
    │   ├── themes.js
    │   ├── plugins.js
    │   ├── registry.js
    │   ├── workflows.js
    │   ├── cron.js
    │   ├── intros.js
    │   └── system.js
    └── components/
        ├── sidebar.js
        ├── toast.js
        ├── modal.js
        ├── toggle.js
        └── color-picker.js
```

## Design Principles

1. **Zero cost when not running** — dashboard code is compiled into the binary but
   only executes when `lx dashboard` is invoked. No background server, no daemon.

2. **No business logic in the dashboard** — every mutation calls existing library
   functions (`mutate_config_transaction`, `install_tool_via_pm`, `generate_tool_plugin`,
   etc.). The dashboard is a UI layer, not a logic layer.

3. **SSE for real-time updates** — the server broadcasts typed events
   (`{type: "theme", data: ...}`) via Server-Sent Events. The frontend routes
   updates to the correct page. Changes made in the dashboard reflect in the
   shell on the next prompt.

4. **Modular files** — each page, component, and stylesheet is a separate file.
   No god files. AI agents and contributors can edit one page without touching others.

5. **Dark theme with design tokens** — CSS custom properties define the color palette.
   Changing `--bg-primary` changes the entire UI. Professional, modern, 2026.

## Crate Dependencies

```
lynx-dashboard ← lynx-core, lynx-config, lynx-theme, lynx-registry, lynx-task
                  axum, tower-http, tokio, serde, serde_json, toml
```

Position in the dependency tree: same level as lynx-cli (assembler, not implementor).

## Migration from Studio

`lx theme studio` is deprecated. It prints a notice and launches `lx dashboard`
with the Themes page focused. The lynx-studio crate is removed once all dashboard
pages are complete and verified.
