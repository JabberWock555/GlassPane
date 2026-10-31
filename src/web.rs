//! HTTP + WebSocket server for dashboard clients (the tablet, phones, PCs).

use crate::background::Backgrounds;
use crate::config::Config;
use crate::ha::Command;
use crate::hub::Hub;
use crate::photos::{Fit, Photos, Target};
use crate::store::Store;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, mpsc};
use tower_http::services::ServeDir;
use tracing::{debug, warn};

#[derive(Clone)]
pub struct AppState {
    pub cfg: Arc<Config>,
    pub hub: Arc<Hub>,
    pub cmds: mpsc::Sender<Command>,
    pub toggleable: Arc<HashSet<String>>,
    pub fans: Arc<HashSet<String>>,
    pub todos: Arc<HashSet<String>>,
    pub bg: Arc<Backgrounds>,
    pub store: Arc<Store>,
    pub photos: Arc<Photos>,
    pub started: Instant,
}

pub fn router(state: AppState) -> Router {
    let static_files = ServeDir::new(&state.cfg.web_dir);
    Router::new()
        .route("/api/layout", get(layout))
        .route("/api/settings", get(get_settings).put(put_settings))
        .route("/api/usage", get(usage))
        .route("/api/info", get(info))
        .route("/api/photos", get(photo_list))
        .route("/photo/{idx}", get(photo))
        .route("/photo/{idx}/blur", get(photo_blur))
        .route("/ws", get(ws_upgrade))
        .route("/bg.jpg", get(bg_sharp))
        .route("/bg-blur.jpg", get(bg_blurred))
        .fallback_service(static_files)
        .layer(DefaultBodyLimit::max(16 * 1024))
        .with_state(state)
}

async fn layout(State(s): State<AppState>) -> impl IntoResponse {
    Json(json!({ "pages": s.cfg.pages, "shopping_list": s.cfg.shopping_list }))
}

async fn get_settings(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.store.settings())
}

/// Partial update: `{"screensaver": {"enabled": true}}` changes just that key.
async fn put_settings(State(s): State<AppState>, Json(patch): Json<Value>) -> Response {
    let Some(obj) = patch.as_object() else {
        return (StatusCode::BAD_REQUEST, "expected a JSON object").into_response();
    };
    match s.store.update_settings(obj) {
        Ok(v) => Json(v).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

async fn usage(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.store.usage())
}

/// Facts for the Settings page's info section.
async fn info(State(s): State<AppState>) -> impl IntoResponse {
    let loc = s.hub.location.read().unwrap().clone();
    Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_s": s.started.elapsed().as_secs(),
        "ha_connected": s.hub.is_connected(),
        "ha_version": *s.hub.ha_version.read().unwrap(),
        "location": loc.map(|l| json!({"name": l.name, "lat": l.lat, "lon": l.lon})),
        "aqi": s.hub.aqi(),
        "entities": s.cfg.entities().len(),
        "photos": s.photos.list().len(),
        "photos_dir": s.photos.dir().display().to_string(),
    }))
}

async fn photo_list(State(s): State<AppState>) -> impl IntoResponse {
    Json(json!({ "count": s.photos.list().len(), "dir": s.photos.dir().display().to_string() }))
}

/// Query for `/photo/{idx}`: the screen's pixel size and how to fit the photo.
#[derive(Deserialize)]
struct PhotoQuery {
    w: Option<u32>,
    h: Option<u32>,
    fit: Option<String>,
}

async fn photo(
    State(s): State<AppState>,
    Path(idx): Path<usize>,
    Query(q): Query<PhotoQuery>,
) -> Response {
    let fit = q.fit.as_deref().and_then(Fit::parse).unwrap_or(Fit::Cover);
    let target = Target::new(q.w.unwrap_or(1280), q.h.unwrap_or(800), fit);
    let photos = s.photos.clone();
    serve_photo(idx, move || photos.get(idx, target)).await
}

async fn photo_blur(State(s): State<AppState>, Path(idx): Path<usize>) -> Response {
    let photos = s.photos.clone();
    serve_photo(idx, move || photos.get_blurred(idx)).await
}

/// Image work is CPU-heavy, so it runs on the blocking thread pool.
async fn serve_photo(
    idx: usize,
    job: impl FnOnce() -> anyhow::Result<Vec<u8>> + Send + 'static,
) -> Response {
    match tokio::task::spawn_blocking(job).await {
        Ok(Ok(bytes)) => jpeg(&bytes),
        Ok(Err(e)) => {
            warn!("photo {idx}: {e:#}");
            StatusCode::NOT_FOUND.into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

fn jpeg(bytes: &[u8]) -> Response {
    (
        [
            (header::CONTENT_TYPE, "image/jpeg"),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        bytes.to_vec(),
    )
        .into_response()
}

async fn bg_sharp(State(s): State<AppState>) -> Response {
    jpeg(&s.bg.sharp)
}

async fn bg_blurred(State(s): State<AppState>) -> Response {
    jpeg(&s.bg.blurred)
}

async fn ws_upgrade(ws: WebSocketUpgrade, State(s): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| client(socket, s))
}

/// Messages the dashboard may send.
#[derive(Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
enum ClientMsg {
    Toggle {
        id: String,
    },
    Speed {
        id: String,
        pct: u8,
    },
    #[serde(rename = "todo_add")]
    TodoAdd {
        id: String,
        text: String,
    },
    #[serde(rename = "todo_set")]
    TodoSet {
        id: String,
        uid: String,
        done: bool,
    },
    #[serde(rename = "todo_clear")]
    TodoClear {
        id: String,
    },
}

/// Longest shopping-list item accepted from a client.
const MAX_ITEM_CHARS: usize = 120;

async fn client(mut socket: WebSocket, s: AppState) {
    // Subscribe before taking the snapshot so no update falls in the gap.
    let mut updates = s.hub.subscribe();
    if socket
        .send(Message::Text(s.hub.snapshot_json().into()))
        .await
        .is_err()
    {
        return;
    }
    // Browsers answer pings automatically; a failed send tells us a sleeping
    // tablet's socket is dead so the task can end.
    let mut ping = tokio::time::interval(std::time::Duration::from_secs(25));
    ping.tick().await;
    loop {
        tokio::select! {
            _ = ping.tick() => {
                if socket.send(Message::Ping(Default::default())).await.is_err() {
                    return;
                }
            }
            update = updates.recv() => {
                let text = match update {
                    Ok(u) => u.to_json(),
                    // Slow client missed messages: just resend everything.
                    Err(broadcast::error::RecvError::Lagged(_)) => s.hub.snapshot_json(),
                    Err(broadcast::error::RecvError::Closed) => return,
                };
                if socket.send(Message::Text(text.into())).await.is_err() {
                    return;
                }
            }
            incoming = socket.recv() => {
                let Some(Ok(msg)) = incoming else { return };
                let Message::Text(text) = msg else { continue };
                match serde_json::from_str::<ClientMsg>(&text) {
                    Ok(ClientMsg::Toggle { id }) if s.toggleable.contains(&id) => {
                        debug!("client toggle {id}");
                        s.store.bump(&id);
                        let _ = s.cmds.try_send(Command::Toggle(id));
                    }
                    Ok(ClientMsg::Toggle { id }) => warn!("rejected toggle for non-dashboard entity {id}"),
                    Ok(ClientMsg::Speed { id, pct }) if pct <= 100 && s.fans.contains(&id) => {
                        debug!("client speed {id} {pct}%");
                        s.store.bump(&id);
                        let _ = s.cmds.try_send(Command::SetPercentage(id, pct));
                    }
                    Ok(ClientMsg::Speed { id, pct }) => warn!("rejected speed {pct} for {id}"),
                    Ok(ClientMsg::TodoAdd { id, text }) if s.todos.contains(&id) => {
                        let text = text.trim();
                        if !text.is_empty() && text.chars().count() <= MAX_ITEM_CHARS {
                            let _ = s.cmds.try_send(Command::TodoAdd(id, text.to_string()));
                        } else {
                            warn!("rejected to-do item of {} chars", text.chars().count());
                        }
                    }
                    Ok(ClientMsg::TodoSet { id, uid, done }) if s.todos.contains(&id) && !uid.is_empty() && uid.len() <= 128 => {
                        let _ = s.cmds.try_send(Command::TodoSet(id, uid, done));
                    }
                    Ok(ClientMsg::TodoClear { id }) if s.todos.contains(&id) => {
                        let _ = s.cmds.try_send(Command::TodoClear(id));
                    }
                    Ok(ClientMsg::TodoAdd { id, .. } | ClientMsg::TodoSet { id, .. } | ClientMsg::TodoClear { id }) => {
                        warn!("rejected to-do command for {id}");
                    }
                    Err(_) => debug!("ignored client message: {text}"),
                }
            }
        }
    }
}
