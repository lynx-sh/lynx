//! Workflow API endpoints.
//!
//! B19 (lynx-workflow crate) is not yet complete.
//! All endpoints return 501 until the workflow engine is available.

use axum::{http::StatusCode, response::IntoResponse, Json};

fn not_implemented() -> impl IntoResponse {
    (StatusCode::NOT_IMPLEMENTED, Json(serde_json::json!({
        "error": "workflow engine not yet available (pending B19)"
    })))
}

/// GET /api/workflows — list available workflows.
pub async fn list_workflows() -> impl IntoResponse { not_implemented() }

/// POST /api/workflow/run — start a workflow.
pub async fn run_workflow() -> impl IntoResponse { not_implemented() }

/// GET /api/jobs — list running and recent jobs.
pub async fn list_jobs() -> impl IntoResponse { not_implemented() }
