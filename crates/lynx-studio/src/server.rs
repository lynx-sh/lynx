//! Route handlers for the Lynx Theme Studio axum server.
//!
//! API surface:
//!   GET  /           → embedded HTML frontend
//!   GET  /theme      → active theme as JSON (raw toml::Value)
//!   POST /theme/patch           → apply dot-path scalar mutation
//!   POST /theme/segment         → add/remove/move segment (array op)
//!   POST /theme/segment-order   → replace full order for a side
//!   POST /theme/apply           → write final TOML and signal exit
//!   POST /theme/reset           → reload from disk (undo in-memory edits)
//!   GET  /events     → SSE stream; pushes theme JSON on every change
//!   GET  /colors     → named color registry (for picker suggestions)

use std::sync::Arc;

use anyhow::Result;
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Json, Router,
};
use lynx_config::load as load_config;
use lynx_theme::{
    loader::{builtin_content, load as load_theme, user_theme_dir},
    patch::{self, Side},
};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Mutex};
use tokio_stream::{wrappers::BroadcastStream, StreamExt as _};

static FRONTEND_HTML: &str = include_str!("frontend.html");

// ─── State ────────────────────────────────────────────────────────────────────

/// Shared server state — Mutex protects the working TOML string.
pub struct AppState {
    /// Working TOML content (may differ from disk if user hasn't applied yet).
    pub working: Mutex<String>,
    /// Name of the active theme.
    pub theme_name: String,
    /// Path to the user-writable TOML file.
    pub theme_path: std::path::PathBuf,
    /// Snapshot taken on server start — used by /theme/reset.
    pub snapshot: String,
    /// Broadcast channel — sends updated theme JSON to SSE subscribers.
    pub tx: broadcast::Sender<String>,
}

impl AppState {
    pub fn new() -> Result<Arc<Self>> {
        let cfg = load_config().map_err(|e| anyhow::anyhow!("config load failed: {e}"))?;
        let theme_name = cfg.active_theme.clone();

        // Ensure user-writable copy exists.
        let theme_path = resolve_or_copy_builtin(&theme_name)?;
        let content = std::fs::read_to_string(&theme_path)?;

        let (tx, _) = broadcast::channel(16);

        Ok(Arc::new(Self {
            working: Mutex::new(content.clone()),
            theme_name,
            theme_path,
            snapshot: content,
            tx,
        }))
    }
}

fn resolve_or_copy_builtin(name: &str) -> Result<std::path::PathBuf> {
    let user_path = user_theme_dir().join(format!("{name}.toml"));
    if user_path.exists() {
        return Ok(user_path);
    }
    // Copy built-in to user dir.
    let dir = user_theme_dir();
    std::fs::create_dir_all(&dir)?;
    let content = builtin_content(name)
        .ok_or_else(|| anyhow::anyhow!("built-in theme '{name}' not found"))?;
    // Validate before writing.
    load_theme(name)?;
    std::fs::write(&user_path, content)?;
    Ok(user_path)
}

// ─── Router ───────────────────────────────────────────────────────────────────

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(get_frontend))
        .route("/theme", get(get_theme))
        .route("/theme/patch", post(post_patch))
        .route("/theme/segment", post(post_segment))
        .route("/theme/segment-order", post(post_segment_order))
        .route("/theme/apply", post(post_apply))
        .route("/theme/reset", post(post_reset))
        .route("/events", get(get_events))
        .route("/colors", get(get_colors))
        .with_state(state)
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

async fn get_frontend() -> impl IntoResponse {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(FRONTEND_HTML.to_string())
        .unwrap()
}

async fn get_theme(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let toml_str = state.working.lock().await.clone();
    match toml::from_str::<toml::Value>(&toml_str) {
        Ok(v) => Json(v).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
struct PatchRequest {
    path: String,
    value: String,
}

async fn post_patch(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PatchRequest>,
) -> impl IntoResponse {
    let mut guard = state.working.lock().await;
    let patched = match patch::apply_patch(&guard, &req.path, &req.value) {
        Ok(p) => p,
        Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    };
    // Validate by parsing as theme schema.
    match load_from_path_str(&patched, &state.theme_name) {
        Ok(_) => {
            *guard = patched.clone();
            drop(guard);
            broadcast_theme(&state, &patched);
            match toml::from_str::<toml::Value>(&patched) {
                Ok(v) => Json(v).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
            }
        }
        Err(e) => (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
struct SegmentRequest {
    op: String,   // "add" | "remove" | "move"
    name: String,
    side: Option<String>,
    after: Option<String>,
}

async fn post_segment(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SegmentRequest>,
) -> impl IntoResponse {
    let mut guard = state.working.lock().await;
    let patched = match req.op.as_str() {
        "add" => {
            let side: Side = match req.side.as_deref().unwrap_or("left").parse() {
                Ok(s) => s,
                Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
            };
            match patch::segment_add(&guard, &req.name, side, req.after.as_deref()) {
                Ok(p) => p,
                Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
            }
        }
        "remove" => match patch::segment_remove(&guard, &req.name) {
            Ok(p) => p,
            Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
        },
        "move" => {
            let side: Side = match req.side.as_deref().unwrap_or("left").parse() {
                Ok(s) => s,
                Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
            };
            match patch::segment_move(&guard, &req.name, side, req.after.as_deref()) {
                Ok(p) => p,
                Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
            }
        }
        _ => return (StatusCode::BAD_REQUEST, "unknown op".to_string()).into_response(),
    };
    match load_from_path_str(&patched, &state.theme_name) {
        Ok(_) => {
            *guard = patched.clone();
            drop(guard);
            broadcast_theme(&state, &patched);
            match toml::from_str::<toml::Value>(&patched) {
                Ok(v) => Json(v).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
            }
        }
        Err(e) => (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
struct SegmentOrderRequest {
    side: String,
    order: Vec<String>,
}

async fn post_segment_order(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SegmentOrderRequest>,
) -> impl IntoResponse {
    let side: Side = match req.side.parse() {
        Ok(s) => s,
        Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    };
    let path = side.dot_path();
    let mut guard = state.working.lock().await;

    // Apply each name as a full array rebuild: remove all, then append in order.
    // Simpler: parse, mutate the array directly, re-serialize.
    let patched = match set_array_order(&guard, path, &req.order) {
        Ok(p) => p,
        Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    };
    match load_from_path_str(&patched, &state.theme_name) {
        Ok(_) => {
            *guard = patched.clone();
            drop(guard);
            broadcast_theme(&state, &patched);
            StatusCode::OK.into_response()
        }
        Err(e) => (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()).into_response(),
    }
}

fn set_array_order(content: &str, dot_path: &str, order: &[String]) -> Result<String> {
    let mut root: toml::Value =
        toml::from_str(content).map_err(|e| anyhow::anyhow!("TOML parse: {e}"))?;
    let parts: Vec<&str> = dot_path.split('.').collect();
    set_arr_at(&mut root, &parts, order)?;
    toml::to_string_pretty(&root).map_err(|e| anyhow::anyhow!("TOML serialize: {e}"))
}

fn set_arr_at(node: &mut toml::Value, parts: &[&str], order: &[String]) -> Result<()> {
    if parts.len() == 1 {
        match node {
            toml::Value::Table(t) => {
                t.insert(
                    parts[0].to_string(),
                    toml::Value::Array(
                        order
                            .iter()
                            .map(|s| toml::Value::String(s.clone()))
                            .collect(),
                    ),
                );
                Ok(())
            }
            _ => anyhow::bail!("non-table"),
        }
    } else {
        match node {
            toml::Value::Table(t) => {
                let child = t
                    .get_mut(parts[0])
                    .ok_or_else(|| anyhow::anyhow!("key '{}' not found", parts[0]))?;
                set_arr_at(child, &parts[1..], order)
            }
            _ => anyhow::bail!("non-table at '{}'", parts[0]),
        }
    }
}

async fn post_apply(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let guard = state.working.lock().await;
    let content = guard.clone();
    drop(guard);

    match std::fs::write(&state.theme_path, &content) {
        Ok(_) => {
            // Signal shutdown by sending a ctrl-c equivalent.
            // The server uses graceful_shutdown on ctrl_c — we trigger it via
            // a dedicated channel. Simplest: just kill ourselves.
            tokio::spawn(async {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                std::process::exit(0);
            });
            StatusCode::OK.into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn post_reset(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut guard = state.working.lock().await;
    *guard = state.snapshot.clone();
    let content = guard.clone();
    drop(guard);
    broadcast_theme(&state, &content);
    StatusCode::OK.into_response()
}

async fn get_events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl futures_core::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = state.tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| {
        msg.ok().map(|data| Ok::<Event, std::convert::Infallible>(Event::default().data(data)))
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[derive(Serialize)]
struct ColorEntry {
    name: String,
    hex: String,
}

async fn get_colors() -> impl IntoResponse {
    use lynx_theme::color::named_to_rgb;
    let names = [
        "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white", "grey",
        "light-red", "light-green", "light-yellow", "light-blue", "light-magenta", "light-cyan",
        "orange", "pink", "purple", "brown", "navy", "teal", "lime",
    ];
    let colors: Vec<ColorEntry> = names
        .iter()
        .filter_map(|&name| {
            named_to_rgb(name).map(|(r, g, b)| ColorEntry {
                name: name.to_string(),
                hex: format!("#{r:02x}{g:02x}{b:02x}"),
            })
        })
        .collect();
    Json(colors)
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Validate TOML content by parsing as a theme (uses in-memory parse, not disk I/O).
fn load_from_path_str(
    content: &str,
    name: &str,
) -> std::result::Result<(), lynx_core::error::LynxError> {
    lynx_theme::loader::parse_and_validate(content, name).map(|_| ())
}

fn broadcast_theme(state: &AppState, toml_str: &str) {
    if let Ok(v) = toml::from_str::<toml::Value>(toml_str) {
        if let Ok(json) = serde_json::to_string(&v) {
            let _ = state.tx.send(json);
        }
    }
}
