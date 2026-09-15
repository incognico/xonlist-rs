mod activity;
mod config;
mod geo;
mod heatmap;
mod model;
mod protocol;
mod qfont;
mod refresh;
mod state;
mod view;
mod web;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use clap::Parser;
use tokio::net::{TcpListener, UnixListener};
use tracing::info;

use crate::activity::ActivityDb;
use crate::config::Config;
use crate::geo::Geo;
use crate::model::{init_regex, parse_banned, Snapshot};
use crate::refresh::{load_heatmap, load_snapshot};
use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "xonlist=info,tower_http=info".into()),
        )
        .init();

    let config = Config::parse();
    fs::create_dir_all(&config.data_dir)?;
    init_regex();

    let http = reqwest::Client::builder()
        .user_agent("xonlist/0.1")
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()?;

    let geo = Geo::open(&config.geodb);
    let activity = ActivityDb::open(&config.activity_path())?;
    let snapshot = load_snapshot(&config).unwrap_or_else(Snapshot::empty);
    let heatmap = load_heatmap(&config);
    let bans = std::fs::read_to_string(config.bans_path())
        .map(|t| parse_banned(&t))
        .unwrap_or_default();

    let state = AppState::new(config.clone(), snapshot, geo, activity, heatmap, bans, http);

    tokio::spawn(refresh::scheduler(state.clone()));

    let app = web::router(state);

    if let Some(socket) = &config.socket {
        serve_unix(socket, app).await
    } else {
        info!(addr = %config.listen, "listening on tcp");
        let listener = TcpListener::bind(config.listen).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
}

async fn serve_unix(path: &Path, app: axum::Router) -> anyhow::Result<()> {
    if path.exists() {
        fs::remove_file(path)?;
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let listener = UnixListener::bind(path)?;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o660));
    info!(path = %path.display(), "listening on unix socket");
    axum::serve(listener, app).await?;
    Ok(())
}
