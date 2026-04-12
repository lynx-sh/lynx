//! Plugin API endpoints.

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;

use crate::server::AppState;

/// GET /api/plugins — list enabled plugins with details.
pub async fn list_plugins() -> impl IntoResponse {
    let cfg = match lynx_config::load() {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    let plugins: Vec<serde_json::Value> = cfg
        .enabled_plugins
        .iter()
        .map(|name| {
            let manifest = load_manifest(name);
            serde_json::json!({
                "name": name,
                "enabled": true,
                "description": manifest.as_ref().map(|m| m.description.as_str()).unwrap_or(""),
                "version": manifest.as_ref().map(|m| m.version.as_str()).unwrap_or("?"),
            })
        })
        .collect();

    Json(serde_json::json!({ "plugins": plugins })).into_response()
}

struct ManifestInfo {
    description: String,
    version: String,
}

fn load_manifest(name: &str) -> Option<ManifestInfo> {
    let path = lynx_core::paths::installed_plugins_dir()
        .join(name)
        .join(lynx_core::brand::PLUGIN_MANIFEST);
    let content = std::fs::read_to_string(path).ok()?;
    let val: toml::Value = toml::from_str(&content).ok()?;
    let plugin = val.get("plugin")?;
    Some(ManifestInfo {
        description: plugin
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        version: plugin
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_string(),
    })
}

#[derive(Deserialize)]
pub struct PluginNameRequest {
    pub name: String,
}

/// POST /api/plugin/enable — enable an installed plugin.
pub async fn enable_plugin(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PluginNameRequest>,
) -> impl IntoResponse {
    match lynx_config::enable_plugin(&req.name) {
        Ok(_) => {
            state.broadcast("plugins_updated");
            StatusCode::OK.into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

/// POST /api/plugin/disable — disable a plugin.
pub async fn disable_plugin(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PluginNameRequest>,
) -> impl IntoResponse {
    match lynx_config::disable_plugin(&req.name) {
        Ok(_) => {
            state.broadcast("plugins_updated");
            StatusCode::OK.into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}
