//! Registry API endpoints — browse, install, tap management.

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;

use crate::server::AppState;

/// GET /api/registry — browse the merged tap index.
pub async fn browse() -> impl IntoResponse {
    let taps_path = lynx_core::paths::taps_config_path();
    let tap_list = match lynx_registry::tap::load_taps(&taps_path) {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    match lynx_registry::tap::merge_tap_indexes(&tap_list) {
        Ok(entries) => {
            let items: Vec<serde_json::Value> = entries
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "name": e.entry.name,
                        "description": e.entry.description,
                        "type": format!("{:?}", e.entry.package_type).to_lowercase(),
                        "tap": e.tap_name,
                        "trust": format!("{:?}", e.trust).to_lowercase(),
                    })
                })
                .collect();
            Json(serde_json::json!({ "entries": items })).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// GET /api/taps — list configured taps.
pub async fn list_taps() -> impl IntoResponse {
    let taps_path = lynx_core::paths::taps_config_path();
    match lynx_registry::tap::load_taps(&taps_path) {
        Ok(list) => {
            let items: Vec<serde_json::Value> = list
                .taps
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "name": t.name,
                        "url": t.url,
                        "trust": format!("{:?}", t.trust).to_lowercase(),
                    })
                })
                .collect();
            Json(serde_json::json!({ "taps": items })).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
pub struct TapAddRequest {
    pub name: String,
    pub url: String,
}

/// POST /api/tap/add — add a new tap.
pub async fn add_tap(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TapAddRequest>,
) -> impl IntoResponse {
    let taps_path = lynx_core::paths::taps_config_path();
    let mut list = match lynx_registry::tap::load_taps(&taps_path) {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    if let Err(e) = lynx_registry::tap::add_tap(&mut list, &req.name, &req.url) {
        return (StatusCode::BAD_REQUEST, e.to_string()).into_response();
    }
    if let Err(e) = lynx_registry::tap::save_taps(&list, &taps_path) {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }

    state.broadcast("taps_updated");
    StatusCode::OK.into_response()
}

#[derive(Deserialize)]
pub struct TapRemoveRequest {
    pub name: String,
}

/// POST /api/tap/remove — remove a tap.
pub async fn remove_tap(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TapRemoveRequest>,
) -> impl IntoResponse {
    let taps_path = lynx_core::paths::taps_config_path();
    let mut list = match lynx_registry::tap::load_taps(&taps_path) {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    if let Err(e) = lynx_registry::tap::remove_tap(&mut list, &req.name) {
        return (StatusCode::BAD_REQUEST, e.to_string()).into_response();
    }
    if let Err(e) = lynx_registry::tap::save_taps(&list, &taps_path) {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }

    state.broadcast("taps_updated");
    StatusCode::OK.into_response()
}

#[derive(Deserialize)]
pub struct InstallRequest {
    pub name: String,
}

/// POST /api/plugin/install — install a plugin from registry.
pub async fn install_plugin(
    State(state): State<Arc<AppState>>,
    Json(req): Json<InstallRequest>,
) -> impl IntoResponse {
    let opts = lynx_registry::fetch::FetchOptions::default();
    match lynx_registry::fetch::fetch_plugin(&req.name, &opts) {
        Ok(_path) => {
            // Enable the plugin after install
            let _ = lynx_config::enable_plugin(&req.name);
            state.broadcast("plugins_updated");
            StatusCode::OK.into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}
