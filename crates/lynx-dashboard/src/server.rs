//! Route handlers for the Lynx Dashboard axum server.

use std::sync::Arc;

use axum::{
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    routing::{get, post},
    Json, Router,
};
use tokio::sync::broadcast;
use tokio_stream::{wrappers::BroadcastStream, StreamExt as _};

use crate::api;

// ─── State ───────────────────────────────────────────────────────────────────

/// Shared server state.
pub struct AppState {
    /// Broadcast channel for SSE — pushes update notifications to all subscribers.
    pub tx: broadcast::Sender<String>,
}

impl AppState {
    pub fn new() -> Arc<Self> {
        let (tx, _) = broadcast::channel(64);
        Arc::new(Self { tx })
    }

    /// Broadcast an event type to all SSE subscribers.
    pub fn broadcast(&self, event_type: &str) {
        let _ = self.tx.send(serde_json::json!({ "type": event_type }).to_string());
    }
}

// ─── Router ──────────────────────────────────────────────────────────────────

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        // Frontend assets
        .route("/", get(crate::frontend::index))
        .route("/css/base.css", get(crate::frontend::base_css))
        .route("/css/layout.css", get(crate::frontend::layout_css))
        .route("/css/components.css", get(crate::frontend::components_css))
        .route("/css/pages.css", get(crate::frontend::pages_css))
        .route("/js/app.js", get(crate::frontend::app_js))
        .route("/js/api.js", get(crate::frontend::api_js))
        .route("/js/components/sidebar.js", get(crate::frontend::sidebar_js))
        .route("/js/components/color-picker.js", get(crate::frontend::color_picker_js))
        .route("/js/pages/overview.js", get(crate::frontend::overview_js))
        .route("/js/pages/themes.js", get(crate::frontend::themes_js))
        .route("/js/pages/plugins.js", get(crate::frontend::plugins_js))
        .route("/js/pages/registry.js", get(crate::frontend::registry_js))
        .route("/js/pages/workflows.js", get(crate::frontend::workflows_js))
        .route("/js/pages/cron.js", get(crate::frontend::cron_js))
        .route("/js/pages/intros.js", get(crate::frontend::intros_js))
        .route("/js/pages/system.js", get(crate::frontend::system_js))
        // API info
        .route("/api/info", get(get_info))
        // Config
        .route("/api/config", get(api::config::get_config))
        .route("/api/config/update", post(api::config::update_config))
        // Themes
        .route("/api/theme", get(api::themes::get_theme))
        .route("/api/themes", get(api::themes::list_themes))
        .route("/api/theme/patch", post(api::themes::patch_theme))
        .route("/api/theme/segment", post(api::themes::segment_op))
        .route("/api/theme/segment-order", post(api::themes::segment_order))
        // Plugins
        .route("/api/plugins", get(api::plugins::list_plugins))
        .route("/api/plugin/enable", post(api::plugins::enable_plugin))
        .route("/api/plugin/disable", post(api::plugins::disable_plugin))
        // Colors (for color picker suggestions)
        .route("/api/colors", get(api::system::get_colors))
        // System
        .route("/api/doctor", get(api::system::get_doctor))
        .route("/api/diag", get(api::system::get_diag))
        // Registry
        .route("/api/registry", get(api::registry::browse))
        .route("/api/taps", get(api::registry::list_taps))
        .route("/api/tap/add", post(api::registry::add_tap))
        .route("/api/tap/remove", post(api::registry::remove_tap))
        .route("/api/plugin/install", post(api::registry::install_plugin))
        // Intros
        .route("/api/intros", get(api::intros::list_intros))
        .route("/api/intro/preview", get(api::intros::preview_intro))
        .route("/api/intro/set", post(api::intros::set_intro))
        // Workflows (B19 pending — returns 501)
        .route("/api/workflows", get(api::workflows::list_workflows))
        .route("/api/jobs", get(api::workflows::list_jobs))
        .route("/api/workflow/run", post(api::workflows::run_workflow))
        .route("/api/job/:id", get(api::workflows::get_job))
        .route("/api/job/:id/stream", get(api::workflows::stream_job))
        // Cron
        .route("/api/cron", get(api::cron::list_tasks))
        .route("/api/cron/add", post(api::cron::add_task))
        .route("/api/cron/remove", post(api::cron::remove_task))
        // Theme apply/reset
        .route("/api/theme/apply", post(api::themes::apply_theme))
        .route("/api/theme/reset", post(api::themes::reset_theme))
        // SSE
        .route("/api/events", get(get_events))
        .with_state(state)
}

// ─── Handlers ────────────────────────────────────────────────────────────────

async fn get_info() -> impl IntoResponse {
    Json(serde_json::json!({
        "name": "lynx-dashboard",
        "version": env!("CARGO_PKG_VERSION"),
        "status": "ok"
    }))
}

async fn get_events(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> Sse<impl futures_core::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = state.tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| {
        msg.ok()
            .map(|data| Ok::<Event, std::convert::Infallible>(Event::default().data(data)))
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}
