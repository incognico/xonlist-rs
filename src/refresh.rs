use std::sync::Arc;
use std::time::{Duration, Instant};

use tracing::{info, warn};

use crate::config::Config;
use crate::model::{
    assemble_snapshot, build_server, file_mtime_epoch, now_epoch, parse_banned, Snapshot,
};
use crate::protocol::{query_all_masters, query_servers};
use crate::state::AppState;

pub fn cache_fresh(path: &std::path::Path, ttl: Duration) -> bool {
    let Some(mtime) = file_mtime_epoch(path) else {
        return false;
    };
    let age = now_epoch().saturating_sub(mtime);
    age < ttl.as_secs()
}

pub fn cache_age(path: &std::path::Path) -> Option<Duration> {
    let mtime = file_mtime_epoch(path)?;
    Some(Duration::from_secs(now_epoch().saturating_sub(mtime)))
}

pub async fn load_or_fetch_bans(state: &AppState) -> Vec<String> {
    let path = state.config.bans_path();
    if cache_fresh(&path, state.config.bans_ttl) {
        if let Ok(text) = std::fs::read_to_string(&path) {
            info!(path = %path.display(), "using cached ban list");
            return parse_banned(&text);
        }
    }
    match fetch_bans(state).await {
        Ok(bans) => bans,
        Err(e) => {
            warn!(error = %e, "ban list download failed");
            if let Ok(text) = std::fs::read_to_string(&path) {
                parse_banned(&text)
            } else {
                Vec::new()
            }
        }
    }
}

async fn fetch_bans(state: &AppState) -> anyhow::Result<Vec<String>> {
    info!(url = %state.config.bans_url, "downloading ban list");
    let text = state
        .http
        .get(&state.config.bans_url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    if let Some(parent) = state.config.bans_path().parent() {
        std::fs::create_dir_all(parent)?;
    }
    atomic_write(&state.config.bans_path(), text.as_bytes())?;
    Ok(parse_banned(&text))
}

pub async fn refresh_servers(state: &AppState) -> anyhow::Result<()> {
    let start = Instant::now();
    let bans = state.bans.read().clone();
    let masters = query_all_masters(&state.config.masters).await;
    info!(count = masters.len(), "master servers returned");
    let raw = query_servers(masters, state.config.retries, state.config.query_timeout_ms).await;
    info!(online = raw.len(), "game servers answered");

    let mut servers = Vec::new();
    for r in raw {
        if let Some(s) = build_server(r, &bans) {
            servers.push(s);
        }
    }

    let epoch = now_epoch();
    let snap = assemble_snapshot(servers, epoch);
    info!(
        servers = snap.info.totalservers,
        players = snap.info.totalplayers,
        elapsed_ms = start.elapsed().as_millis() as u64,
        "server list refreshed"
    );

    if let Ok(json) = serde_json::to_vec(&snap) {
        let _ = atomic_write(&state.config.snapshot_path(), &json);
    }

    *state.snapshot.write() = Arc::new(snap);
    Ok(())
}

pub fn load_snapshot(config: &Config) -> Option<Snapshot> {
    let path = config.snapshot_path();
    let data = std::fs::read(&path).ok()?;
    serde_json::from_slice(&data).ok()
}

pub fn record_activity(state: &AppState) {
    let snap = state.snapshot.read().clone();
    match state.activity.lock().record(&snap) {
        Ok(()) => info!("activity recorded"),
        Err(e) => warn!(error = %e, "activity record failed"),
    }
}

pub fn render_heatmap(state: &AppState) {
    let rows = match state.activity.lock().all() {
        Ok(r) => r,
        Err(e) => {
            warn!(error = %e, "activity read failed");
            return;
        }
    };
    match crate::heatmap::render(rows) {
        Ok(png) => {
            let _ = atomic_write(&state.config.heatmap_path(), &png);
            *state.heatmap.write() = Some(png);
            info!("heatmap rendered");
        }
        Err(e) => warn!(error = %e, "heatmap render failed"),
    }
}

pub fn load_heatmap(config: &Config) -> Option<Vec<u8>> {
    std::fs::read(config.heatmap_path()).ok()
}

fn atomic_write(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path)
}

pub fn next_delay(age: Option<Duration>, ttl: Duration) -> Duration {
    match age {
        Some(age) if age < ttl => ttl - age,
        _ => Duration::from_secs(0),
    }
}

pub async fn scheduler(state: AppState) {
    {
        let bans = load_or_fetch_bans(&state).await;
        *state.bans.write() = bans;
    }

    let snap_fresh = cache_fresh(&state.config.snapshot_path(), state.config.server_ttl);
    if !snap_fresh {
        if let Err(e) = refresh_servers(&state).await {
            warn!(error = %e, "initial server refresh failed");
        }
    } else {
        info!("using cached server snapshot");
    }

    if state.snapshot.read().info.totalservers > 0 {
        record_activity(&state);
        render_heatmap(&state);
    } else if state.heatmap.read().is_none() {
        render_heatmap(&state);
    }

    let mut server_wait = next_delay(
        cache_age(&state.config.snapshot_path()),
        state.config.server_ttl,
    );
    if server_wait.is_zero() {
        server_wait = state.config.server_ttl;
    }
    let mut bans_wait = next_delay(cache_age(&state.config.bans_path()), state.config.bans_ttl);
    if bans_wait.is_zero() {
        bans_wait = state.config.bans_ttl;
    }

    let mut server_tick = tokio::time::interval_at(
        tokio::time::Instant::now() + server_wait,
        state.config.server_ttl,
    );
    server_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let mut bans_tick = tokio::time::interval_at(
        tokio::time::Instant::now() + bans_wait,
        state.config.bans_ttl,
    );
    bans_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let mut activity_tick = tokio::time::interval_at(
        tokio::time::Instant::now() + state.config.activity_interval,
        state.config.activity_interval,
    );
    activity_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = server_tick.tick() => {
                if let Err(e) = refresh_servers(&state).await {
                    warn!(error = %e, "server refresh failed; keeping previous snapshot");
                }
            }
            _ = bans_tick.tick() => {
                match fetch_bans(&state).await {
                    Ok(bans) => {
                        *state.bans.write() = bans;
                    }
                    Err(e) => warn!(error = %e, "ban list refresh failed"),
                }
            }
            _ = activity_tick.tick() => {
                record_activity(&state);
                render_heatmap(&state);
            }
        }
    }
}
