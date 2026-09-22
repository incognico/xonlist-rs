use std::collections::HashSet;

use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderName, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use rust_embed::RustEmbed;
use serde::Deserialize;
use tower_http::trace::TraceLayer;

use crate::model::Snapshot;
use crate::state::AppState;
use crate::view::{
    views_from_snapshot, EmbedTemplate, IndexTemplate, RowsTemplate, ServerView, SiteCtx,
};

#[derive(RustEmbed)]
#[folder = "static"]
struct Assets;

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

fn site_from(state: &AppState, q: &CommonQuery) -> SiteCtx {
    SiteCtx::from_config(&state.config, q.rjz.is_some())
}

fn live_snapshot(state: &AppState) -> Snapshot {
    let mut snap = state.snapshot.read().with_lastupdate_now();
    state.geo.apply(&mut snap);
    snap
}

async fn index(State(state): State<AppState>, Query(q): Query<CommonQuery>) -> Response {
    let snap = live_snapshot(&state);
    let site = site_from(&state, &q);
    let servers = views_from_snapshot(&snap);
    let tmpl = IndexTemplate {
        site: &site,
        totalplayers: snap.info.totalplayers,
        totalbots: snap.info.totalbots,
        activeservers: snap.info.activeservers,
        totalservers: snap.info.totalservers,
        lastupdate: snap.info.lastupdate,
        servers: &servers,
        embed: false,
    };
    render(tmpl)
}

async fn server_page(
    State(state): State<AppState>,
    Path(server): Path<String>,
    Query(q): Query<CommonQuery>,
) -> Response {
    let snap = live_snapshot(&state);
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
    let snap = live_snapshot(&state);
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
    let snap = live_snapshot(&state);
    let body = if q.pretty.is_some() {
        serde_json::to_vec_pretty(&snap).unwrap_or_else(|_| b"{}".to_vec())
    } else {
        serde_json::to_vec(&snap).unwrap_or_else(|_| b"{}".to_vec())
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
        Some(f) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                [(
                    header::CONTENT_TYPE,
                    HeaderValue::from_str(mime.as_ref())
                        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
                )],
                f.data.into_owned(),
            )
                .into_response()
        }
        None => not_found(),
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
