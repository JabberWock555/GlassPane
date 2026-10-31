//! glasspane — a Home Assistant wall dashboard that runs on very old tablets.
//!
//! The server sits between Home Assistant and the dashboard screens:
//!
//! ```text
//! Home Assistant ⇄ WebSocket ⇄ glasspane ⇄ HTTP + WebSocket ⇄ tablet / phone / PC
//! ```
//!
//! It holds the HA token, keeps the live state of the dashboard's entities,
//! and serves a small frontend plus pre-rendered images, so the tablet's old
//! browser only has light work to do. Background tasks:
//! - `ha::run`      — the Home Assistant connection (auto-reconnecting)
//! - `aqi::run`     — air quality for the home location every 20 minutes
//! - `photos::warm` — pre-renders screensaver photos for known screen sizes

mod aqi;
mod background;
mod config;
mod ha;
mod hub;
mod photos;
mod store;
mod web;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "glasspane=info".into()),
        )
        .init();

    let path: PathBuf = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "glasspane.toml".into())
        .into();
    let cfg = Arc::new(config::Config::load(&path)?);

    // The long-lived HA token only ever lives on the server, never the tablet.
    let token = std::env::var("HA_TOKEN").map_err(|_| {
        anyhow::anyhow!("HA_TOKEN is not set (create a long-lived access token in your HA profile)")
    })?;

    let hub = Arc::new(hub::Hub::new(cfg.entities()));
    let (cmd_tx, cmd_rx) = mpsc::channel(32);
    tokio::spawn(ha::run(
        cfg.ha_url.clone(),
        token,
        hub.clone(),
        cfg.todos(),
        cmd_rx,
    ));
    tokio::spawn(aqi::run(hub.clone(), cfg.aqi_location));

    let (bg_path, sigma) = (cfg.background.clone(), cfg.blur_sigma);
    let bg =
        tokio::task::spawn_blocking(move || background::build(bg_path.as_deref(), sigma)).await??;

    let data_dir = Path::new(&cfg.data_dir);
    let photos = Arc::new(photos::Photos::new(&cfg.photos_dir, data_dir));
    tokio::spawn(photos::warm(photos.clone()));
    let state = web::AppState {
        toggleable: Arc::new(cfg.toggleable()),
        fans: Arc::new(cfg.fans()),
        todos: Arc::new(cfg.todos().into_iter().collect()),
        store: Arc::new(store::Store::open(data_dir)),
        photos,
        started: Instant::now(),
        cfg: cfg.clone(),
        hub,
        cmds: cmd_tx,
        bg: Arc::new(bg),
    };

    let listener = tokio::net::TcpListener::bind(&cfg.listen).await?;
    info!("glasspane listening on http://{}", cfg.listen);
    axum::serve(listener, web::router(state)).await?;
    Ok(())
}
