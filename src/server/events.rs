//! SSE endpoint (/api/events) and SQLite data_version polling.

use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use serde_json::json;
use tokio::sync::broadcast;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

use super::AppState;

/// Spawns a background task polling SQLite PRAGMA data_version every ~500ms.
pub fn start_poller(db_path: PathBuf, tx_change: broadcast::Sender<()>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(500));
        let mut conn_opt: Option<rusqlite::Connection> = None;
        let mut last_version = 0i64;

        loop {
            interval.tick().await;
            if conn_opt.is_none()
                && let Ok(c) = rusqlite::Connection::open(&db_path)
            {
                let _ = c.pragma_update(None, "journal_mode", "WAL");
                let _ = c.pragma_update(None, "busy_timeout", 5000);
                if let Ok(v) = c.query_row("PRAGMA data_version", [], |r| r.get(0)) {
                    last_version = v;
                }
                conn_opt = Some(c);
            }

            if let Some(conn) = &conn_opt {
                match conn.query_row("PRAGMA data_version", [], |r| r.get::<_, i64>(0)) {
                    Ok(current_version) => {
                        if current_version != last_version {
                            last_version = current_version;
                            let _ = tx_change.send(());
                        }
                    }
                    Err(_) => {
                        conn_opt = None;
                    }
                }
            }
        }
    });
}

/// GET /api/events
/// Server-Sent Events stream: emits {type:'changed'} when SQLite data_version changes, plus keepalive.
pub async fn events_handler(
    State(state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.tx_change.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|res| match res {
        Ok(()) => Some(Ok(
            Event::default().data(json!({ "type": "changed" }).to_string())
        )),
        Err(_) => None,
    });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    )
}
