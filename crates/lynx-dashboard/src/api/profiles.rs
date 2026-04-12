//! Profile API endpoints.

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;

use crate::server::AppState;

/// GET /api/profiles — list available profiles.
pub async fn list_profiles() -> impl IntoResponse {
    let cfg = match lynx_config::load() {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    let names = match lynx_config::profile::list_names() {
        Ok(n) => n,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    let active = cfg.active_profile.as_deref().unwrap_or("");
    let items: Vec<serde_json::Value> = names
        .iter()
        .map(|n| {
            serde_json::json!({
                "name": n,
                "active": n == active,
            })
        })
        .collect();

    Json(serde_json::json!({ "profiles": items })).into_response()
}

#[derive(Deserialize)]
pub struct ProfileSetRequest {
    pub name: String,
}

/// POST /api/profile/set — switch the active profile.
pub async fn set_profile(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ProfileSetRequest>,
) -> impl IntoResponse {
    // Validate profile exists
    if lynx_config::profile::load(&req.name).is_err() {
        return (
            StatusCode::NOT_FOUND,
            format!("profile '{}' not found", req.name),
        )
            .into_response();
    }

    let result =
        lynx_config::snapshot::mutate_config_transaction("dashboard-profile-set", |cfg| {
            cfg.active_profile = Some(req.name.clone());
            Ok(())
        });

    match result {
        Ok(_) => {
            state.broadcast("profile_updated");
            StatusCode::OK.into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
