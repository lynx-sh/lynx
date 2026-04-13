//! Workflow and jobs API endpoints.

use axum::{
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    Json,
};
use std::collections::HashMap;

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

/// POST /api/workflow/run — start a workflow in background, return job_id.
pub async fn run_workflow(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    let name = match body.get("name").and_then(|v| v.as_str()) {
        Some(n) => n.to_string(),
        None => {
            return Json(serde_json::json!({
                "error": "missing 'name' field"
            }));
        }
    };

    // Load workflow
    let workflow = match lynx_workflow::store::load_workflow(&name) {
        Ok(w) => w,
        Err(e) => {
            return Json(serde_json::json!({
                "error": format!("workflow '{}' not found: {}", name, e)
            }));
        }
    };

    // Parse params from request body
    let params: HashMap<String, String> = body
        .get("params")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();

    // Spawn execution in background
    let log_dir = Some(lynx_core::paths::jobs_dir());
    tokio::spawn(async move {
        let _ = lynx_workflow::executor::execute_workflow(
            &workflow,
            &params,
            lynx_workflow::executor::ExecMode::Background,
            log_dir,
        )
        .await;
    });

    Json(serde_json::json!({
        "status": "started",
        "workflow": name,
    }))
}

/// GET /api/job/:id — get job result by ID.
pub async fn get_job(
    axum::extract::Path(job_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    match lynx_workflow::jobs::get_job(&job_id) {
        Ok(val) => Json(val),
        Err(_) => Json(serde_json::json!({ "error": format!("job '{}' not found", job_id) })),
    }
}

/// GET /api/job/:id/stream — SSE stream of job log output.
///
/// Polls the job's .log file for new lines and streams them as SSE events.
/// Sends a "done" event when the job's .json result file appears (job completed).
pub async fn stream_job(
    axum::extract::Path(job_id): axum::extract::Path<String>,
) -> Sse<impl futures_core::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let stream = async_stream::stream! {
        let jobs_dir = lynx_core::paths::jobs_dir();
        let log_path = jobs_dir.join(format!("{job_id}.log"));
        let result_path = jobs_dir.join(format!("{job_id}.json"));
        let mut offset: u64 = 0;

        loop {
            // Read new log lines since last offset
            if log_path.exists() {
                if let Ok(content) = tokio::fs::read_to_string(&log_path).await {
                    let bytes = content.as_bytes();
                    if (offset as usize) < bytes.len() {
                        let new_data = &content[offset as usize..];
                        for line in new_data.lines() {
                            if !line.is_empty() {
                                yield Ok::<Event, std::convert::Infallible>(
                                    Event::default().event("log").data(line)
                                );
                            }
                        }
                        offset = bytes.len() as u64;
                    }
                }
            }

            // Check if job is done
            if result_path.exists() {
                if let Ok(content) = tokio::fs::read_to_string(&result_path).await {
                    yield Ok(Event::default().event("done").data(content));
                }
                break;
            }

            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
}
