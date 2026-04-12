//! Embedded frontend assets served as typed HTTP responses.

use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};

static INDEX_HTML: &str = include_str!("index.html");
static BASE_CSS: &str = include_str!("css/base.css");
static LAYOUT_CSS: &str = include_str!("css/layout.css");
static COMPONENTS_CSS: &str = include_str!("css/components.css");
static APP_JS: &str = include_str!("js/app.js");
static API_JS: &str = include_str!("js/api.js");
static SIDEBAR_JS: &str = include_str!("js/components/sidebar.js");

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

pub async fn app_js() -> impl IntoResponse {
    serve(APP_JS, "application/javascript; charset=utf-8")
}

pub async fn api_js() -> impl IntoResponse {
    serve(API_JS, "application/javascript; charset=utf-8")
}

pub async fn sidebar_js() -> impl IntoResponse {
    serve(SIDEBAR_JS, "application/javascript; charset=utf-8")
}
