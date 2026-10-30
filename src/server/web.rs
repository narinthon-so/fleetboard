//! Static asset serving from embedded web/dist using rust-embed.

use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "web/dist"]
pub struct Assets;

/// Serve embedded files from web/dist with SPA fallback for client-side routing.
pub async fn static_handler(uri: Uri) -> Response {
    let raw_path = uri.path();
    // Do not serve web assets or SPA fallback for /api routes
    if raw_path.starts_with("/api") {
        return (
            StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({
                "v": 1,
                "ok": false,
                "error": {
                    "code": "NOT_FOUND",
                    "message": "endpoint not found"
                }
            })),
        )
            .into_response();
    }

    let mut path = raw_path.trim_start_matches('/').to_string();
    if path.is_empty() {
        path = "index.html".to_string();
    }

    match Assets::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
        }
        None => {
            // SPA fallback: if the path has no file extension (e.g. /task/fb-12), serve index.html
            if !path.contains('.')
                && let Some(index) = Assets::get("index.html")
            {
                return (
                    [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                    index.data,
                )
                    .into_response();
            }
            StatusCode::NOT_FOUND.into_response()
        }
    }
}
