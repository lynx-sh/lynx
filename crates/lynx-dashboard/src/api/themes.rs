//! Theme API endpoints.

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use lynx_theme::{
    loader::user_theme_dir,
    patch::{self, Side},
};
use serde::Deserialize;

use crate::server::AppState;

/// GET /api/themes — list available theme names.
pub async fn list_themes() -> impl IntoResponse {
    Json(serde_json::json!({ "themes": lynx_theme::list() }))
}

/// GET /api/theme — return the active theme as JSON.
pub async fn get_theme() -> impl IntoResponse {
    let cfg = match lynx_config::load() {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    let theme_path = user_theme_dir().join(format!("{}.toml", cfg.active_theme));
    match std::fs::read_to_string(&theme_path) {
        Ok(content) => match toml::from_str::<toml::Value>(&content) {
            Ok(v) => Json(v).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        Err(e) => (StatusCode::NOT_FOUND, e.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
pub struct ThemePatchRequest {
    pub path: String,
    pub value: String,
}

/// POST /api/theme/patch — apply a dot-path scalar mutation.
pub async fn patch_theme(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ThemePatchRequest>,
) -> impl IntoResponse {
    let cfg = match lynx_config::load() {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    let theme_path = user_theme_dir().join(format!("{}.toml", cfg.active_theme));
    let content = match std::fs::read_to_string(&theme_path) {
        Ok(c) => c,
        Err(e) => return (StatusCode::NOT_FOUND, e.to_string()).into_response(),
    };

    let patched = match patch::apply_patch(&content, &req.path, &req.value) {
        Ok(p) => p,
        Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    };

    // Validate before writing
    if let Err(e) = lynx_theme::parse_and_validate(&patched, &cfg.active_theme) {
        return (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()).into_response();
    }

    if let Err(e) = std::fs::write(&theme_path, &patched) {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }

    state.broadcast("theme_updated");
    match toml::from_str::<toml::Value>(&patched) {
        Ok(v) => Json(v).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
pub struct SegmentRequest {
    pub op: String,
    pub name: String,
    pub side: Option<String>,
    pub after: Option<String>,
}

/// POST /api/theme/segment — add/remove/move a segment.
pub async fn segment_op(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SegmentRequest>,
) -> impl IntoResponse {
    let cfg = match lynx_config::load() {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    let theme_path = user_theme_dir().join(format!("{}.toml", cfg.active_theme));
    let content = match std::fs::read_to_string(&theme_path) {
        Ok(c) => c,
        Err(e) => return (StatusCode::NOT_FOUND, e.to_string()).into_response(),
    };

    let patched = match req.op.as_str() {
        "add" => {
            let side: Side = match req.side.as_deref().unwrap_or("left").parse() {
                Ok(s) => s,
                Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
            };
            patch::segment_add(&content, &req.name, side, req.after.as_deref())
        }
        "remove" => patch::segment_remove(&content, &req.name),
        "move" => {
            let side: Side = match req.side.as_deref().unwrap_or("left").parse() {
                Ok(s) => s,
                Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
            };
            patch::segment_move(&content, &req.name, side, req.after.as_deref())
        }
        _ => return (StatusCode::BAD_REQUEST, "unknown op".to_string()).into_response(),
    };

    match patched {
        Ok(p) => {
            if let Err(e) = lynx_theme::parse_and_validate(&p, &cfg.active_theme) {
                return (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()).into_response();
            }
            if let Err(e) = std::fs::write(&theme_path, &p) {
                return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
            }
            state.broadcast("theme_updated");
            match toml::from_str::<toml::Value>(&p) {
                Ok(v) => Json(v).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
            }
        }
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
pub struct SegmentOrderRequest {
    pub side: String,
    pub order: Vec<String>,
}

/// POST /api/theme/segment-order — replace full segment order for a side.
pub async fn segment_order(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SegmentOrderRequest>,
) -> impl IntoResponse {
    let cfg = match lynx_config::load() {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    let theme_path = user_theme_dir().join(format!("{}.toml", cfg.active_theme));
    let content = match std::fs::read_to_string(&theme_path) {
        Ok(c) => c,
        Err(e) => return (StatusCode::NOT_FOUND, e.to_string()).into_response(),
    };

    let side: Side = match req.side.parse() {
        Ok(s) => s,
        Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    };
    let dot_path = side.dot_path();

    // Parse, set array, re-serialize
    let mut root: toml::Value = match toml::from_str(&content) {
        Ok(v) => v,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    if let Err(e) = set_arr_at(&mut root, dot_path, &req.order) {
        return (StatusCode::BAD_REQUEST, e.to_string()).into_response();
    }

    let patched = match toml::to_string_pretty(&root) {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    if let Err(e) = lynx_theme::parse_and_validate(&patched, &cfg.active_theme) {
        return (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()).into_response();
    }
    if let Err(e) = std::fs::write(&theme_path, &patched) {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }

    state.broadcast("theme_updated");
    StatusCode::OK.into_response()
}

/// POST /api/theme/apply — save theme (already written by patch, this is a no-op confirmation).
pub async fn apply_theme(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // Theme patches already write to disk immediately.
    // This endpoint confirms the apply action.
    state.broadcast("theme_applied");
    StatusCode::OK.into_response()
}

/// POST /api/theme/reset — reload theme from disk (undo unsaved changes).
pub async fn reset_theme(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    state.broadcast("theme_updated");
    StatusCode::OK.into_response()
}

fn set_arr_at(
    node: &mut toml::Value,
    dot_path: &str,
    order: &[String],
) -> Result<(), String> {
    let parts: Vec<&str> = dot_path.split('.').collect();
    let mut current = node;
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            if let toml::Value::Table(t) = current {
                t.insert(
                    part.to_string(),
                    toml::Value::Array(
                        order.iter().map(|s| toml::Value::String(s.clone())).collect(),
                    ),
                );
                return Ok(());
            }
            return Err("non-table parent".into());
        }
        current = match current {
            toml::Value::Table(t) => t
                .get_mut(*part)
                .ok_or_else(|| format!("key '{part}' not found"))?,
            _ => return Err(format!("non-table at '{part}'")),
        };
    }
    Err("empty path".into())
}
