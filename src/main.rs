mod activity;
mod assets;
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

use tokio::net::{TcpListener, UnixListener};
use tracing::info;
use tracing_subscriber::filter::{LevelFilter, Targets};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::activity::ActivityDb;
use crate::config::Config;
use crate::geo::Geo;
use crate::model::{parse_banned, Snapshot};
use crate::refresh::{load_heatmap, load_snapshot};
use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let level = env_log_level();
    tracing_subscriber::registry()
        .with(
            Targets::new()
                .with_target("xonlist", level)
                .with_target("tower_http", level)
                .with_default(LevelFilter::WARN),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::parse();
    fs::create_dir_all(&config.data_dir)?;

    let http = reqwest::Client::builder()
        .user_agent("xonlist/0.1")
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()?;

    let geo = Geo::open(config.geodb.clone());
    let activity = ActivityDb::open(&config.activity_path())?;
    let mut snapshot = load_snapshot(&config).unwrap_or_else(Snapshot::empty);
    geo.apply(&mut snapshot);
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

fn env_log_level() -> LevelFilter {
    match std::env::var("RUST_LOG").ok().as_deref() {
        Some("error") | Some("xonlist=error") => LevelFilter::ERROR,
        Some("warn") | Some("xonlist=warn") => LevelFilter::WARN,
        Some("debug") | Some("xonlist=debug") => LevelFilter::DEBUG,
        Some("trace") | Some("xonlist=trace") => LevelFilter::TRACE,
        _ => LevelFilter::INFO,
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
