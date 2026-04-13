//! Intro API endpoints.

use std::collections::HashMap;
use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;

use crate::server::AppState;

/// GET /api/intros — list available intros.
pub async fn list_intros() -> impl IntoResponse {
    let cfg = match lynx_config::load() {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    let entries = lynx_intro::loader::list_all();
    let active = cfg.intro.active.as_deref().unwrap_or("");

    let items: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "slug": e.slug,
                "name": e.name,
                "builtin": e.is_builtin,
                "active": e.slug == active,
            })
        })
        .collect();

    Json(serde_json::json!({
        "intros": items,
        "enabled": cfg.intro.enabled,
    }))
    .into_response()
}

/// GET /api/intro/preview?slug=<slug> — preview an intro.
pub async fn preview_intro(
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let slug = match params.get("slug") {
        Some(s) => s.as_str(),
        None => return (StatusCode::BAD_REQUEST, "missing slug param".to_string()).into_response(),
    };

    let intro = match lynx_intro::loader::load(slug) {
        Ok(i) => i,
        Err(e) => return (StatusCode::NOT_FOUND, e.to_string()).into_response(),
    };

    let tokens = lynx_intro::tokens::build_token_map(&HashMap::new());
    let rendered = lynx_intro::renderer::render_intro(&intro, &tokens);

    Json(serde_json::json!({
        "slug": slug,
        "rendered": rendered,
    }))
    .into_response()
}

#[derive(Deserialize)]
pub struct IntroSetRequest {
    pub slug: String,
}

/// POST /api/intro/set — switch the active intro.
pub async fn set_intro(
    State(state): State<Arc<AppState>>,
    Json(req): Json<IntroSetRequest>,
) -> impl IntoResponse {
    // Validate slug exists
    if lynx_intro::loader::load(&req.slug).is_err() {
        return (
            StatusCode::NOT_FOUND,
            format!("intro '{}' not found", req.slug),
        )
            .into_response();
    }

    let result = lynx_config::snapshot::mutate_config_transaction("dashboard-intro-set", |cfg| {
        cfg.intro.active = Some(req.slug.clone());
        cfg.intro.enabled = true;
        Ok(())
    });

    match result {
        Ok(_) => {
            state.broadcast("intros_updated");
            StatusCode::OK.into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
