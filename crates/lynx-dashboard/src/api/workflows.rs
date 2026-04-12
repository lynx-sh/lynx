//! Workflow and jobs API endpoints.

use axum::{response::IntoResponse, Json};

/// GET /api/workflows — list available workflows.
pub async fn list_workflows() -> impl IntoResponse {
    match lynx_workflow::store::list_workflows() {
        Ok(entries) => {
            let items: Vec<serde_json::Value> = entries
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "name": e.name,
                        "description": e.description,
                    })
                })
                .collect();
            Json(serde_json::json!({ "workflows": items }))
        }
        Err(_) => Json(serde_json::json!({ "workflows": [] })),
    }
}

/// GET /api/jobs — list recent jobs.
pub async fn list_jobs() -> impl IntoResponse {
    match lynx_workflow::jobs::list_jobs() {
        Ok(entries) => {
            let items: Vec<serde_json::Value> = entries
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "job_id": e.job_id,
                        "workflow": e.workflow,
                        "success": e.success,
                        "started_at": e.started_at,
                        "duration_ms": e.duration_ms,
                    })
                })
                .collect();
            Json(serde_json::json!({ "jobs": items }))
        }
        Err(_) => Json(serde_json::json!({ "jobs": [] })),
    }
}

/// POST /api/workflow/run — start a workflow (placeholder — execution via CLI).
pub async fn run_workflow() -> impl IntoResponse {
    Json(serde_json::json!({
        "error": "workflow execution via API not yet supported — use `lx run <name>`"
    }))
}
