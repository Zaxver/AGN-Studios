//! WebUI server — serves embedded static HTML files.
//!
//! The webui is a single-page HTML application stored in the `webui/` directory.
//! It is embedded into the binary at compile time using rust-embed.

use crate::SharedState;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../webui/"]
struct WebuiAssets;

/// Serve the webui index.html (or any static asset).
/// Uses `Option<Path<String>>` so root `/` (no path) also works.
pub async fn serve_webui(path: Option<axum::extract::Path<String>>) -> Response {
    // If a path was provided, try to serve that specific asset
    if let Some(axum::extract::Path(p)) = path {
        // Ignore query strings
        let clean = p.split('?').next().unwrap_or(&p);
        if !clean.is_empty() {
            if let Some(content) = WebuiAssets::get(clean) {
                let mime = mime_guess::from_path(clean).first_or_octet_stream();
                return (
                    StatusCode::OK,
                    [("content-type", mime.as_ref())],
                    content.data.to_vec(),
                ).into_response();
            }
        }
    }

    // Default to index.html (SPA fallback)
    if let Some(content) = WebuiAssets::get("index.html") {
        return (
            StatusCode::OK,
            [("content-type", "text/html; charset=utf-8")],
            content.data.to_vec(),
        ).into_response();
    }

    (StatusCode::NOT_FOUND, "WebUI not found").into_response()
}