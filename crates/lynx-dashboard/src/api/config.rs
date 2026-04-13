//! Config API endpoints.

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;

use crate::server::AppState;

/// GET /api/config — return current config as JSON.
pub async fn get_config(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    match lynx_config::load() {
        Ok(cfg) => {
            let val = serde_json::json!({
                "schema_version": cfg.schema_version,
                "active_theme": cfg.active_theme,
                "active_context": format!("{:?}", cfg.active_context).to_lowercase(),
                "enabled_plugins": cfg.enabled_plugins,
            });
            Json(val).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
pub struct ConfigUpdateRequest {
    pub active_theme: Option<String>,
    pub active_context: Option<String>,
}

/// POST /api/config/update — update config fields.
pub async fn update_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ConfigUpdateRequest>,
) -> impl IntoResponse {
    let result =
        lynx_config::snapshot::mutate_config_transaction("dashboard-config-update", |cfg| {
            if let Some(ref theme) = req.active_theme {
                cfg.active_theme = theme.clone();
            }
            if let Some(ref ctx) = req.active_context {
                cfg.active_context = match ctx.as_str() {
                    "agent" => lynx_core::types::Context::Agent,
                    "minimal" => lynx_core::types::Context::Minimal,
                    _ => lynx_core::types::Context::Interactive,
                };
            }
            Ok(())
        });

    match result {
        Ok(_) => {
            state.broadcast("config_updated");
            StatusCode::OK.into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}
