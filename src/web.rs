use std::collections::HashSet;
use std::sync::Arc;

use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderName, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde::Deserialize;
use serde::Serialize;
use tower_http::trace::TraceLayer;

use crate::assets::Assets;
use crate::model::{now_epoch, Info, Snapshot};
use crate::state::AppState;
use crate::view::{
    views_from_snapshot, EmbedTemplate, IndexTemplate, RowsTemplate, ServerView, SiteCtx,
};

const AGE_TOKEN: &str = "@XONLIST_AGE@";

#[derive(Deserialize)]
pub struct CommonQuery {
    pub rjz: Option<String>,
    pub pretty: Option<String>,
    #[serde(default)]
    pub s: Vec<String>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/server/{server}", get(server_page))
        .route("/servers", get(servers_page))
        .route("/endpoint/json", get(json_endpoint))
        .route("/heatmap.png", get(heatmap))
        .fallback(static_or_404)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[derive(Serialize)]
struct JsonOut<'a> {
    info: Info,
    server: &'a std::collections::BTreeMap<String, crate::model::Server>,
}

fn site_from(state: &AppState, q: &CommonQuery) -> SiteCtx {
    SiteCtx::from_config(&state.config, q.rjz.is_some())
}

fn current_snapshot(state: &AppState) -> (Arc<Snapshot>, u64) {
    if state.geo.reload_if_changed() {
        let mut guard = state.snapshot.write();
        let mut snap = (**guard).clone();
        state.geo.apply(&mut snap);
        *guard = Arc::new(snap);
    }
    let snap = state.snapshot.read().clone();
    let age = now_epoch().saturating_sub(snap.info.lastupdate_epoch);
    (snap, age)
}

fn render_index(state: &AppState, snap: &Snapshot, rjz: bool) -> String {
    let site = SiteCtx::from_config(&state.config, rjz);
    let servers = views_from_snapshot(snap);
    let tmpl = IndexTemplate {
        site: &site,
        totalplayers: snap.info.totalplayers,
        totalbots: snap.info.totalbots,
        activeservers: snap.info.activeservers,
        totalservers: snap.info.totalservers,
        servers: &servers,
        embed: false,
    };
    tmpl.render().unwrap_or_else(|e| {
        tracing::error!(error = %e, "template render failed");
        String::new()
    })
}

async fn index(State(state): State<AppState>, Query(q): Query<CommonQuery>) -> Response {
    let (snap, age) = current_snapshot(&state);
    let ptr = Arc::as_ptr(&snap) as usize;
    let rjz = q.rjz.is_some();
    let cached = {
        let mut pages = state.pages.lock();
        if pages.ptr != ptr {
            pages.plain = render_index(&state, &snap, false);
            pages.rjz = render_index(&state, &snap, true);
            pages.ptr = ptr;
        }
        if rjz {
            pages.rjz.clone()
        } else {
            pages.plain.clone()
        }
    };
    Html(cached.replacen(AGE_TOKEN, &age.to_string(), 1)).into_response()
}

async fn server_page(
    State(state): State<AppState>,
    Path(server): Path<String>,
    Query(q): Query<CommonQuery>,
) -> Response {
    let (snap, _) = current_snapshot(&state);
    let mut site = site_from(&state, &q);
    site.noindex = true;
    let servers: Vec<ServerView> = snap
        .server
        .get(&server)
        .map(crate::view::server_view)
        .into_iter()
        .collect();
    let tmpl = EmbedTemplate {
        site: &site,
        has_server: !servers.is_empty(),
        servers: &servers,
        embed: true,
    };
    let mut response = render(tmpl);
    response.headers_mut().insert(
        HeaderName::from_static("x-robots-tag"),
        HeaderValue::from_static("noindex"),
    );
    response
}

async fn servers_page(State(state): State<AppState>, Query(q): Query<CommonQuery>) -> Response {
    let (snap, _) = current_snapshot(&state);
    let site = site_from(&state, &q);
    let want: HashSet<&str> = q.s.iter().map(|s| s.as_str()).collect();
    let servers: Vec<ServerView> = snap
        .server
        .values()
        .filter(|s| want.contains(s.address.as_str()))
        .map(crate::view::server_view)
        .collect();
    let tmpl = RowsTemplate {
        site: &site,
        servers: &servers,
        embed: false,
    };
    render(tmpl)
}

async fn json_endpoint(State(state): State<AppState>, Query(q): Query<CommonQuery>) -> Response {
    let (snap, age) = current_snapshot(&state);
    let mut info = snap.info.clone();
    info.lastupdate = age;
    let body = JsonOut {
        info,
        server: &snap.server,
    };
    let body = if q.pretty.is_some() {
        serde_json::to_vec_pretty(&body).unwrap_or_else(|_| b"{}".to_vec())
    } else {
        serde_json::to_vec(&body).unwrap_or_else(|_| b"{}".to_vec())
    };
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        )],
        body,
    )
        .into_response()
}

async fn heatmap(State(state): State<AppState>) -> Response {
    match state.heatmap.read().clone() {
        Some(png) => (
            [(header::CONTENT_TYPE, HeaderValue::from_static("image/png"))],
            png,
        )
            .into_response(),
        None => not_found(),
    }
}

async fn static_or_404(uri: axum::http::Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path.is_empty() {
        return not_found();
    }
    match Assets::get(path) {
        Some(f) => (
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static(content_type(path)),
            )],
            f.data.into_owned(),
        )
            .into_response(),
        None => not_found(),
    }
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("txt") => "text/plain; charset=utf-8",
        Some("html") => "text/html; charset=utf-8",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("eot") => "application/vnd.ms-fontobject",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream",
    }
}

fn not_found() -> Response {
    let body = Assets::get("404.html")
        .map(|f| f.data.into_owned())
        .unwrap_or_else(|| b"404".to_vec());
    (
        StatusCode::NOT_FOUND,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        )],
        body,
    )
        .into_response()
}

fn render(tmpl: impl Template) -> Response {
    match tmpl.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "template render failed");
            let body = Assets::get("500.html")
                .map(|f| f.data.into_owned())
                .unwrap_or_else(|| b"500".to_vec());
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("text/html; charset=utf-8"),
                )],
                body,
            )
                .into_response()
        }
    }
}
