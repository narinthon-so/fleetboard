//! Authentication, session cookies, and X-Requested-With verification.

use std::collections::HashMap;
use std::fmt::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{Request, State};
use axum::http::{self, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;
use subtle::ConstantTimeEq;

use super::AppState;

/// Idle session lifetime. Each valid request pushes expiry this far ahead;
/// after this long with no request the session is rejected and a fresh
/// `fb serve` one-time token is needed.
pub const SESSION_TTL: Duration = Duration::from_secs(60 * 60 * 24);

/// A session id is valid only while present and not past its expiry.
fn session_is_valid(sessions: &HashMap<String, Instant>, id: &str, now: Instant) -> bool {
    sessions.get(id).is_some_and(|exp| *exp > now)
}

/// Generates a cryptographically secure token using the OS CSPRNG (32 bytes, 64 hex chars).
pub fn generate_random_token() -> String {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).expect("OS CSPRNG failed to generate random bytes");
    let mut s = String::with_capacity(64);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Constant-time string comparison to prevent timing attacks.
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    if a_bytes.len() != b_bytes.len() {
        return false;
    }
    a_bytes.ct_eq(b_bytes).into()
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
            Some(id) => {
                // Guard against lock poisoning and drop expired sessions lazily.
                // Valid requests extend the session (sliding TTL), so an actively
                // used board never hits the daily single-token lockout.
                match state.sessions.write() {
                    Ok(mut sessions) => {
                        let now = Instant::now();
                        if session_is_valid(&sessions, &id, now) {
                            sessions.insert(id.clone(), now + SESSION_TTL);
                            true
                        } else {
                            sessions.remove(&id);
                            false
                        }
                    }
                    Err(_) => false,
                }
            }
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

    let mut token_guard = match state.one_time_token.lock() {
        Ok(g) => g,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(json!({
                    "v": 1,
                    "ok": false,
                    "error": {
                        "code": "INTERNAL",
                        "message": "auth store unavailable"
                    }
                })),
            )
                .into_response();
        }
    };
    if let Some(valid_token) = token_guard.as_ref()
        && constant_time_eq(valid_token, &payload.token)
    {
        // One-time: consume the token immediately
        *token_guard = None;

        // Generate a secure session id
        let session_id = generate_random_token();
        if let Ok(mut sessions) = state.sessions.write() {
            sessions.insert(session_id.clone(), Instant::now() + SESSION_TTL);
        } else {
            // Poisoned lock: the one-time token is already spent, so fail
            // instead of returning a cookie that no code will honor.
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(json!({
                    "v": 1,
                    "ok": false,
                    "error": {
                        "code": "INTERNAL",
                        "message": "session store unavailable"
                    }
                })),
            )
                .into_response();
        }

        // Session cookie (no Max-Age): the browser keeps it until it closes,
        // and the server's sliding SESSION_TTL alone decides expiry. A fixed
        // Max-Age would drop the cookie 24h after login however active the
        // board is, and the one-time token is already spent by then.
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
        && let Ok(mut sessions) = state.sessions.write()
    {
        sessions.remove(&session_id);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokens_use_os_csprng_and_have_expected_length_and_differ() {
        let t1 = generate_random_token();
        let t2 = generate_random_token();

        // 32 bytes encoded as hex = 64 hex characters
        assert_eq!(t1.len(), 64);
        assert_eq!(t2.len(), 64);
        // Characters must be lowercase hex digits
        assert!(
            t1.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
        assert!(
            t2.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );

        // Two generated CSPRNG tokens must not collide
        assert_ne!(t1, t2);

        // Constant-time equality checks
        assert!(constant_time_eq(&t1, &t1));
        assert!(constant_time_eq(&t2, &t2));
        assert!(!constant_time_eq(&t1, &t2));
        assert!(!constant_time_eq(&t1, &t1[..32]));
        assert!(!constant_time_eq(&t1, ""));
    }

    #[test]
    fn sessions_expire() {
        let mut sessions = HashMap::new();
        let now = Instant::now();
        sessions.insert("live".to_string(), now + Duration::from_secs(60));
        sessions.insert("expired".to_string(), now - Duration::from_secs(1));
        assert!(session_is_valid(&sessions, "live", now));
        assert!(!session_is_valid(&sessions, "expired", now));
        assert!(!session_is_valid(&sessions, "missing", now));
    }
}
