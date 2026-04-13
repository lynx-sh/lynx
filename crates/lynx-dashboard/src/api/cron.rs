//! Cron/scheduled-task API endpoints.

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;

use crate::server::AppState;

fn load_tasks_file() -> lynx_task::schema::TasksFile {
    let path = lynx_core::paths::tasks_file();
    let content = lynx_task::store::read_tasks_file(&path).unwrap_or_default();
    if content.is_empty() {
        return lynx_task::schema::TasksFile { tasks: vec![] };
    }
    lynx_task::store::parse_tasks_file(&content)
        .unwrap_or(lynx_task::schema::TasksFile { tasks: vec![] })
}

/// GET /api/cron — list scheduled tasks from tasks.toml.
pub async fn list_tasks() -> impl IntoResponse {
    let file = load_tasks_file();
    let items: Vec<serde_json::Value> = file
        .tasks
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name,
                "command": t.run,
                "schedule": t.cron,
                "description": t.description,
            })
        })
        .collect();
    Json(serde_json::json!({ "tasks": items }))
}

#[derive(Deserialize)]
pub struct TaskAddRequest {
    pub name: String,
    pub command: String,
    pub schedule: String,
    #[serde(default)]
    pub description: String,
}

/// POST /api/cron/add — add a scheduled task.
pub async fn add_task(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TaskAddRequest>,
) -> impl IntoResponse {
    let mut file = load_tasks_file();

    if file.tasks.iter().any(|t| t.name == req.name) {
        return (
            StatusCode::CONFLICT,
            format!("task '{}' already exists", req.name),
        )
            .into_response();
    }

    file.tasks.push(lynx_task::schema::Task {
        name: req.name,
        description: req.description,
        run: req.command,
        cron: req.schedule,
        on_fail: Default::default(),
        timeout: None,
        log: true,
        enabled: true,
    });

    let path = lynx_core::paths::tasks_file();
    match lynx_task::store::write_tasks_file(&path, &file) {
        Ok(_) => {
            state.broadcast("cron_updated");
            StatusCode::OK.into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
pub struct TaskNameRequest {
    pub name: String,
}

/// POST /api/cron/remove — remove a scheduled task.
pub async fn remove_task(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TaskNameRequest>,
) -> impl IntoResponse {
    let mut file = load_tasks_file();

    let before = file.tasks.len();
    file.tasks.retain(|t| t.name != req.name);
    if file.tasks.len() == before {
        return (
            StatusCode::NOT_FOUND,
            format!("task '{}' not found", req.name),
        )
            .into_response();
    }

    let path = lynx_core::paths::tasks_file();
    match lynx_task::store::write_tasks_file(&path, &file) {
        Ok(_) => {
            state.broadcast("cron_updated");
            StatusCode::OK.into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
