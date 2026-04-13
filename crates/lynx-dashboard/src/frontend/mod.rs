//! Embedded frontend assets served as typed HTTP responses.

use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};

static INDEX_HTML: &str = include_str!("index.html");
static BASE_CSS: &str = include_str!("css/base.css");
static LAYOUT_CSS: &str = include_str!("css/layout.css");
static COMPONENTS_CSS: &str = include_str!("css/components.css");
static PAGES_CSS: &str = include_str!("css/pages.css");
static APP_JS: &str = include_str!("js/app.js");
static API_JS: &str = include_str!("js/api.js");
static SIDEBAR_JS: &str = include_str!("js/components/sidebar.js");
static COLOR_PICKER_JS: &str = include_str!("js/components/color-picker.js");
static OVERVIEW_JS: &str = include_str!("js/pages/overview.js");
static THEMES_JS: &str = include_str!("js/pages/themes.js");
static PLUGINS_JS: &str = include_str!("js/pages/plugins.js");
static REGISTRY_JS: &str = include_str!("js/pages/registry.js");
static WORKFLOWS_JS: &str = include_str!("js/pages/workflows.js");
static CRON_JS: &str = include_str!("js/pages/cron.js");
static INTROS_JS: &str = include_str!("js/pages/intros.js");
static SYSTEM_JS: &str = include_str!("js/pages/system.js");

fn serve(content: &'static str, content_type: &'static str) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .body(content.to_string())
        .unwrap()
        .into_response()
}

pub async fn index() -> impl IntoResponse {
    serve(INDEX_HTML, "text/html; charset=utf-8")
}

pub async fn base_css() -> impl IntoResponse {
    serve(BASE_CSS, "text/css; charset=utf-8")
}

pub async fn layout_css() -> impl IntoResponse {
    serve(LAYOUT_CSS, "text/css; charset=utf-8")
}

pub async fn components_css() -> impl IntoResponse {
    serve(COMPONENTS_CSS, "text/css; charset=utf-8")
}

pub async fn pages_css() -> impl IntoResponse {
    serve(PAGES_CSS, "text/css; charset=utf-8")
}

pub async fn app_js() -> impl IntoResponse {
    serve(APP_JS, "application/javascript; charset=utf-8")
}

pub async fn api_js() -> impl IntoResponse {
    serve(API_JS, "application/javascript; charset=utf-8")
}

pub async fn sidebar_js() -> impl IntoResponse {
    serve(SIDEBAR_JS, "application/javascript; charset=utf-8")
}

pub async fn color_picker_js() -> impl IntoResponse {
    serve(COLOR_PICKER_JS, "application/javascript; charset=utf-8")
}

pub async fn overview_js() -> impl IntoResponse {
    serve(OVERVIEW_JS, "application/javascript; charset=utf-8")
}

pub async fn themes_js() -> impl IntoResponse {
    serve(THEMES_JS, "application/javascript; charset=utf-8")
}

pub async fn plugins_js() -> impl IntoResponse {
    serve(PLUGINS_JS, "application/javascript; charset=utf-8")
}

pub async fn registry_js() -> impl IntoResponse {
    serve(REGISTRY_JS, "application/javascript; charset=utf-8")
}

pub async fn workflows_js() -> impl IntoResponse {
    serve(WORKFLOWS_JS, "application/javascript; charset=utf-8")
}

pub async fn cron_js() -> impl IntoResponse {
    serve(CRON_JS, "application/javascript; charset=utf-8")
}

pub async fn intros_js() -> impl IntoResponse {
    serve(INTROS_JS, "application/javascript; charset=utf-8")
}

pub async fn system_js() -> impl IntoResponse {
    serve(SYSTEM_JS, "application/javascript; charset=utf-8")
}
