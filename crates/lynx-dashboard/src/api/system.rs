//! System API endpoints — doctor checks and diagnostic log.

use axum::{response::IntoResponse, Json};
use lynx_doctor::Status;

/// GET /api/doctor — run health checks and return results.
pub async fn get_doctor() -> impl IntoResponse {
    let results = lynx_doctor::run_all();
    let items: Vec<serde_json::Value> = results
        .iter()
        .map(|c| {
            let mut obj = serde_json::json!({
                "name": c.name,
                "status": c.status.label(),
                "detail": c.detail,
            });
            if let Some(fix) = &c.fix {
                obj["fix"] = serde_json::Value::String(fix.clone());
            }
            obj
        })
        .collect();

    let any_fail = results.iter().any(|c| c.status == Status::Fail);
    Json(serde_json::json!({
        "checks": items,
        "healthy": !any_fail,
    }))
}

/// GET /api/colors — named color registry for picker suggestions.
pub async fn get_colors() -> impl IntoResponse {
    let names = [
        "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white", "grey",
        "light-red", "light-green", "light-yellow", "light-blue", "light-magenta", "light-cyan",
        "orange", "pink", "purple", "brown", "navy", "teal", "lime",
    ];
    let colors: Vec<serde_json::Value> = names
        .iter()
        .filter_map(|&name| {
            lynx_theme::color::named_to_rgb(name).map(|(r, g, b)| {
                serde_json::json!({
                    "name": name,
                    "hex": format!("#{r:02x}{g:02x}{b:02x}"),
                })
            })
        })
        .collect();
    Json(colors)
}

/// GET /api/diag — return recent diagnostic log entries.
pub async fn get_diag() -> impl IntoResponse {
    let lines = lynx_core::diag::tail(100);
    Json(serde_json::json!({ "lines": lines }))
}
