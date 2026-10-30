//! Authentication, session cookies, and X-Requested-With verification.

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{self, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;

use super::AppState;

pub fn generate_random_token() -> String {
    (0..32).map(|_| fastrand::alphanumeric()).collect()
}

fn extract_session_id(cookie_str: &str) -> Option<String> {
    for cookie in cookie_str.split(';') {
        let trimmed = cookie.trim();
        if let Some(val) = trimmed.strip_prefix("fb_session=")
            && !val.is_empty()
        {
            return Some(val.to_string());
        }
    }
    None
}

/// Middleware that checks:
/// 1. Every /api call (except /api/login) must have a valid session cookie.
/// 2. Write calls (POST, PUT, DELETE, PATCH) also require `X-Requested-With: fleetboard`.
pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path();

    // Only protect /api routes (skipping /api/login)
    if path.starts_with("/api/") && path != "/api/login" {
        let cookie_header = req.headers().get(http::header::COOKIE);
        let session_id = cookie_header
            .and_then(|h| h.to_str().ok())
            .and_then(extract_session_id);

        let is_authenticated = match session_id {
            Some(id) => state.sessions.read().unwrap().contains(&id),
            None => false,
        };

        if !is_authenticated {
            return (
                StatusCode::UNAUTHORIZED,
                axum::Json(json!({
                    "v": 1,
                    "ok": false,
                    "error": {
                        "code": "UNAUTHORIZED",
                        "message": "authentication required"
                    }
                })),
            )
                .into_response();
        }

        // Writes also need X-Requested-With: fleetboard
        let method = req.method();
        if method == http::Method::POST
            || method == http::Method::PUT
            || method == http::Method::DELETE
            || method == http::Method::PATCH
        {
            let x_req = req.headers().get("X-Requested-With");
            let valid = x_req
                .and_then(|h| h.to_str().ok())
                .is_some_and(|v| v.eq_ignore_ascii_case("fleetboard"));

            if !valid {
                return (
                    StatusCode::FORBIDDEN,
                    axum::Json(json!({
                        "v": 1,
                        "ok": false,
                        "error": {
                            "code": "FORBIDDEN",
                            "message": "missing or invalid X-Requested-With header"
                        }
                    })),
                )
                    .into_response();
            }
        }
    }

    next.run(req).await
}

#[derive(Deserialize)]
pub struct LoginPayload {
    pub token: String,
}

/// POST /api/login
/// Exchanges the one-time login token for an HttpOnly SameSite=Strict session cookie.
pub async fn login_handler(
    State(state): State<Arc<AppState>>,
    payload: Result<axum::Json<LoginPayload>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Ok(axum::Json(payload)) = payload else {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(json!({
                "v": 1,
                "ok": false,
                "error": {
                    "code": "USAGE",
                    "message": "expected JSON body with token string"
                }
            })),
        )
            .into_response();
    };

    let mut token_guard = state.one_time_token.lock().unwrap();
    if let Some(valid_token) = token_guard.as_ref()
        && *valid_token == payload.token
    {
        // One-time: consume the token immediately
        *token_guard = None;

        // Generate a secure session id
        let session_id = generate_random_token();
        state.sessions.write().unwrap().insert(session_id.clone());

        let cookie_header = format!("fb_session={session_id}; HttpOnly; SameSite=Strict; Path=/");

        return (
            StatusCode::OK,
            [(http::header::SET_COOKIE, cookie_header)],
            axum::Json(json!({
                "v": 1,
                "ok": true,
                "data": {
                    "message": "logged in"
                }
            })),
        )
            .into_response();
    }

    (
        StatusCode::UNAUTHORIZED,
        axum::Json(json!({
            "v": 1,
            "ok": false,
            "error": {
                "code": "UNAUTHORIZED",
                "message": "invalid or expired token"
            }
        })),
    )
        .into_response()
}

/// POST /api/logout
pub async fn logout_handler(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let cookie_header = req.headers().get(http::header::COOKIE);
    if let Some(session_id) = cookie_header
        .and_then(|h| h.to_str().ok())
        .and_then(extract_session_id)
    {
        state.sessions.write().unwrap().remove(&session_id);
    }

    let cookie_header = "fb_session=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0";
    (
        StatusCode::OK,
        [(http::header::SET_COOKIE, cookie_header)],
        axum::Json(json!({
            "v": 1,
            "ok": true,
            "data": {
                "message": "logged out"
            }
        })),
    )
        .into_response()
}
