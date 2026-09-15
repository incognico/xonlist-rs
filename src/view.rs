use askama::Template;

use crate::config::Config;
use crate::model::{
    encryption_label, mode_tag, ping_kind, sorted_servers, team_class, trunc_with_title, ucfirst,
    PingKind, Server, Snapshot,
};

#[derive(Clone)]
pub struct SiteCtx {
    #[allow(dead_code)]
    pub domain: String,
    pub title: String,
    pub desc: String,
    pub asset_base: String,
    pub rjz: bool,
}

impl SiteCtx {
    pub fn from_config(cfg: &Config, rjz: bool) -> Self {
        Self {
            domain: cfg.domain.clone(),
            title: cfg.title.clone(),
            desc: cfg.desc.clone(),
            asset_base: cfg.asset_base(),
            rjz,
        }
    }
}

#[derive(Clone)]
pub struct TeamScoreView {
    pub class: String,
    pub label: String,
    pub value: i32,
}

#[derive(Clone)]
pub struct PlayerView {
    pub team_class: Option<String>,
    pub name_display: String,
    pub name_title: Option<String>,
    pub ping_arrows: bool,
    pub ping_bot: bool,
    pub ping_num: Option<i32>,
    pub score_display: String,
    pub score_title: Option<String>,
}

#[derive(Clone)]
pub struct ServerView {
    pub address: String,
    pub geo: String,
    pub geo_lower: String,
    pub realname: String,
    pub mode: String,
    pub modefull: String,
    pub mode_tag: String,
    pub instagib_dot: bool,
    pub map_display: String,
    pub map_title: Option<String>,
    pub numplayers: i32,
    pub maxplayers: i32,
    pub empty: bool,
    pub version: String,
    pub gamedir: String,
    pub d0id: String,
    pub encryption: String,
    pub stats_supported: bool,
    pub fullbright_forbidden: bool,
    pub impure: i32,
    pub slots: i32,
    pub team_scores: Vec<TeamScoreView>,
    pub players: Vec<PlayerView>,
    pub has_player_table: bool,
    pub player_query_error: bool,
    pub score_header: String,
}

pub fn server_view(s: &Server) -> ServerView {
    let (mode_tag, instagib_dot) = mode_tag(s);
    let (map_display, map_title) = trunc_with_title(&s.map, 16);
    let mut team_scores = Vec::new();
    if s.teamplay != 0 {
        let prefer = if s.scoreinfo.team.prefer == "sec" {
            &s.scoreinfo.team.sec
        } else {
            &s.scoreinfo.team.pri
        };
        if !prefer.label.is_empty() || !prefer.score.is_empty() {
            for (k, v) in &prefer.score {
                if let Some(class) = team_class(*k) {
                    team_scores.push(TeamScoreView {
                        class: class.into(),
                        label: format!("Team {k} {}", ucfirst(&prefer.label)),
                        value: *v,
                    });
                }
            }
        }
    }

    let players: Vec<PlayerView> = s
        .players
        .iter()
        .map(|p| {
            let (name_display, name_title) = trunc_with_title(&p.name, 42);
            let (score_display, score_title) = crate::model::player_score_display(s, p);
            let (ping_arrows, ping_bot, ping_num) = match ping_kind(p) {
                PingKind::Arrows => (true, false, None),
                PingKind::Bot => (false, true, None),
                PingKind::Number(n) => (false, false, Some(n)),
            };
            PlayerView {
                team_class: if s.teamplay != 0 {
                    team_class(p.team).map(|c| c.to_string())
                } else {
                    None
                },
                name_display,
                name_title,
                ping_arrows,
                ping_bot,
                ping_num,
                score_display,
                score_title,
            }
        })
        .collect();

    let has_player_table = s.numplayers > 0 || s.numbots > 0;
    let player_query_error = has_player_table && players.is_empty();

    let score_header = if s.mode == "COOP" && s.mode2 == "QUAKE" {
        "Deaths".into()
    } else if !s.scoreinfo.player.label.is_empty() {
        ucfirst(&s.scoreinfo.player.label)
    } else {
        "Score".into()
    };

    ServerView {
        address: s.address.clone(),
        geo: s.geo.clone(),
        geo_lower: s.geo.to_lowercase(),
        realname: s.realname.clone(),
        mode: s.mode.clone(),
        modefull: s.modefull.clone(),
        mode_tag,
        instagib_dot,
        map_display,
        map_title,
        numplayers: s.numplayers,
        maxplayers: s.maxplayers,
        empty: s.numplayers < 1,
        version: s.version.clone(),
        gamedir: s.gamedir.clone(),
        d0id: s.d0id.clone(),
        encryption: encryption_label(s.enc).into(),
        stats_supported: s.stats != 0,
        fullbright_forbidden: s.fballowed == 0,
        impure: s.impure,
        slots: s.slots,
        team_scores,
        players,
        has_player_table,
        player_query_error,
        score_header,
    }
}

pub fn views_from_snapshot(snap: &Snapshot) -> Vec<ServerView> {
    sorted_servers(snap).into_iter().map(server_view).collect()
}

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate<'a> {
    pub site: &'a SiteCtx,
    pub totalplayers: i32,
    pub totalbots: i32,
    pub activeservers: i32,
    pub totalservers: i32,
    pub lastupdate: u64,
    pub servers: &'a [ServerView],
    pub embed: bool,
}

#[derive(Template)]
#[template(path = "embed.html")]
pub struct EmbedTemplate<'a> {
    pub site: &'a SiteCtx,
    pub servers: &'a [ServerView],
    pub has_server: bool,
    pub embed: bool,
}

#[derive(Template)]
#[template(path = "rows.html")]
pub struct RowsTemplate<'a> {
    pub site: &'a SiteCtx,
    pub servers: &'a [ServerView],
    pub embed: bool,
}
