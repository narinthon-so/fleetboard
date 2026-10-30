//! axum HTTP server, auth, read API, SSE.

pub mod api;
pub mod auth;
pub mod events;
pub mod web;

use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use axum::Router;
use axum::middleware;
use axum::routing::{get, post};
use tokio::net::TcpListener;
use tokio::sync::broadcast;

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub addr: SocketAddr,
    pub token: String,
    pub home: PathBuf,
}

#[derive(Clone)]
pub struct AppState {
    pub config: ServerConfig,
    pub db_path: PathBuf,
    pub one_time_token: Arc<Mutex<Option<String>>>,
    pub sessions: Arc<RwLock<HashSet<String>>>,
    pub tx_change: broadcast::Sender<()>,
    pub host: String,
}

impl AppState {
    pub fn new(config: ServerConfig) -> anyhow::Result<Arc<Self>> {
        std::fs::create_dir_all(&config.home)?;
        let db_path = config.home.join("board.db");
        // Ensure database schema and WAL mode are initialized
        let _ = crate::store::SqliteBackend::open(&db_path)?;

        let (tx_change, _) = broadcast::channel(64);
        let host = gethostname::gethostname().to_string_lossy().into_owned();

        // Start SQLite data_version poller
        events::start_poller(db_path.clone(), tx_change.clone());

        let state = Arc::new(Self {
            one_time_token: Arc::new(Mutex::new(Some(config.token.clone()))),
            sessions: Arc::new(RwLock::new(HashSet::new())),
            tx_change,
            host,
            db_path,
            config,
        });

        Ok(state)
    }
}

/// Construct the full axum router with routes and auth middleware.
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/login", post(auth::login_handler))
        .route("/api/logout", post(auth::logout_handler))
        .route("/api/board", get(api::board_handler))
        .route("/api/tasks/{id}", get(api::task_detail_handler))
        .route("/api/events", get(events::events_handler))
        .fallback(web::static_handler)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ))
        .with_state(state)
}

/// Run the server loop.
pub async fn run_server(listener: TcpListener, state: Arc<AppState>) -> anyhow::Result<()> {
    let app = router(state);
    axum::serve(listener, app).await?;
    Ok(())
}
