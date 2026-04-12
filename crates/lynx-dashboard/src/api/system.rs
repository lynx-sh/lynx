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

/// GET /api/diag — return recent diagnostic log entries.
pub async fn get_diag() -> impl IntoResponse {
    let lines = lynx_core::diag::tail(100);
    Json(serde_json::json!({ "lines": lines }))
}
