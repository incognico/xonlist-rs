use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::protocol::{RawPlayer, RawStatus};
use crate::qfont::{format_name, ordinate, score2time, truncate_egc};

const MODES: &[(&str, &str)] = &[
    ("ARENA", "Duel Arena"),
    ("AS", "Assault"),
    ("BR", "Battle Royale"),
    ("CA", "Clan Arena"),
    ("CONQUEST", "Conquest"),
    ("COOP", "Cooperative"),
    ("CQ", "Conquest"),
    ("CTF", "Capture the Flag"),
    ("CTS", "Race - Complete the Stage"),
    ("DM", "Deathmatch"),
    ("DOM", "Domination"),
    ("DOTC", "Defense of the Core (MOBA)"),
    ("DUEL", "Duel"),
    ("FT", "Freeze Tag"),
    ("INF", "Infection"),
    ("INV", "Invasion"),
    ("JAILBREAK", "Jailbreak"),
    ("JB", "Jailbreak"),
    ("KA", "Keepaway"),
    ("KH", "Key Hunt"),
    ("LMS", "Last Man Standing"),
    ("MAYHEM", "Mayhem"),
    ("NB", "Nexball"),
    ("ONS", "Onslaught"),
    ("RACE", "Race"),
    ("RC", "Race"),
    ("RUNE", "Runematch"),
    ("RUNEMATCH", "Runematch"),
    ("SNAFU", "???"),
    ("SURV", "Survival"),
    ("TDM", "Team Deathmatch"),
    ("TKA", "Team Keepaway"),
    ("TMAYHEM", "Team Mayhem"),
    ("VIP", "Very Important Player"),
];

fn mode_full(mode: &str) -> String {
    MODES
        .iter()
        .find(|(k, _)| *k == mode)
        .map(|(_, v)| (*v).to_string())
        .unwrap_or_else(|| mode.to_string())
}

fn map_team(raw: i32) -> i32 {
    match raw {
        5 => 1,  // red
        14 => 2, // blue
        13 => 3, // yellow
        10 => 4, // pink
        other => other,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Info {
    #[serde(default)]
    pub activeservers: i32,
    #[serde(default)]
    pub totalservers: i32,
    #[serde(default)]
    pub totalplayers: i32,
    #[serde(default)]
    pub totalbots: i32,
    #[serde(default)]
    pub lastupdate_epoch: u64,
    #[serde(default)]
    pub lastupdate: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScoreFlags {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub flags: String,
    #[serde(default)]
    pub order: i32,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub score: BTreeMap<i32, i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TeamScoreInfo {
    #[serde(default)]
    pub prefer: String,
    #[serde(default)]
    pub pri: ScoreFlags,
    #[serde(default)]
    pub sec: ScoreFlags,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScoreInfo {
    #[serde(default)]
    pub player: ScoreFlags,
    #[serde(default)]
    pub team: TeamScoreInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub name: String,
    pub score: i32,
    pub ping: i32,
    pub team: i32,
    pub prio: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub address: String,
    pub realname: String,
    pub map: String,
    pub geo: String,
    pub version: String,
    pub gamedir: String,
    pub mode: String,
    pub modefull: String,
    pub mode2: String,
    pub impure: i32,
    pub slots: i32,
    pub numplayers: i32,
    pub maxplayers: i32,
    pub numbots: i32,
    pub numspectators: i32,
    pub enc: i32,
    pub d0id: String,
    pub fballowed: i32,
    pub teamplay: i32,
    pub stats: i32,
    #[serde(default)]
    pub scoreinfo: ScoreInfo,
    #[serde(default)]
    pub players: Vec<Player>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Snapshot {
    pub info: Info,
    pub server: BTreeMap<String, Server>,
}

impl Snapshot {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn with_lastupdate_now(&self) -> Self {
        let mut s = self.clone();
        let now = now_epoch();
        s.info.lastupdate = now.saturating_sub(s.info.lastupdate_epoch);
        s
    }
}

pub fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn file_mtime_epoch(path: &std::path::Path) -> Option<u64> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?;
    mtime.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

pub fn parse_banned(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix('B') {
            let rest = rest.trim_start();
            if let Some(host) = rest.strip_suffix(':').or_else(|| {
                rest.split_once(':')
                    .filter(|(_, t)| t.trim().is_empty())
                    .map(|(h, _)| h)
            }) {
                let host = host.trim();
                if !host.is_empty() {
                    out.push(host.to_string());
                }
            }
        }
    }
    out
}

fn rule_str(rules: &std::collections::HashMap<String, Vec<u8>>, key: &str) -> String {
    rules
        .get(key)
        .map(|v| String::from_utf8_lossy(v).into_owned())
        .unwrap_or_default()
}

fn rule_i32(rules: &std::collections::HashMap<String, Vec<u8>>, key: &str) -> i32 {
    rule_str(rules, key).parse().unwrap_or(0)
}

pub fn build_server(raw: RawStatus, geo: String, banned: &[String]) -> Option<Server> {
    let ip = raw.address.ip().to_string();
    if banned.iter().any(|b| b == &ip) {
        return None;
    }

    let gameversion: i64 = rule_str(&raw.rules, "gameversion").parse().unwrap_or(0);
    if gameversion > 65535 {
        return None;
    }

    let mut numbots = rule_i32(&raw.rules, "bots");
    if numbots < 0 {
        numbots = 0;
    }
    let mut numplayers = rule_i32(&raw.rules, "clients");
    if numplayers == 0 && !raw.players.is_empty() {
        numplayers = raw.players.len() as i32;
    }
    numplayers = (numplayers - numbots).max(0);
    let maxplayers = rule_i32(&raw.rules, "sv_maxclients").max(0);

    let hostname = raw
        .rules
        .get("hostname")
        .map(|v| v.as_slice())
        .unwrap_or(b"");
    if hostname.is_empty() {
        return None;
    }

    let d0 = rule_str(&raw.rules, "d0_blind_id");
    let (enc, d0id) = if d0.is_empty() {
        (0, "0".to_string())
    } else {
        match d0.split_once(' ') {
            Some((e, rest)) => (e.parse().unwrap_or(0), rest.to_string()),
            None => (d0.parse().unwrap_or(0), "0".to_string()),
        }
    };

    let qc = rule_str(&raw.rules, "qcstatus");
    let parsed = parse_qcstatus(&qc, numplayers);

    let mut players: Vec<Player> = raw
        .players
        .iter()
        .map(|p| convert_player(p, &parsed))
        .collect();

    let numspectators = players
        .iter()
        .filter(|p| p.score == -666 || p.name == "unconnected")
        .count() as i32;

    let mut mode = parsed.mode.clone();
    if mode == "DM" && numplayers - numspectators == 2 {
        mode = "DUEL".to_string();
    }
    let modefull = mode_full(&mode);

    if parsed.player_order != 0 {
        players.sort_by_key(|p| (p.prio, p.score, p.team));
    } else {
        players.sort_by(|a, b| {
            a.prio
                .cmp(&b.prio)
                .then(a.team.cmp(&b.team))
                .then(b.score.cmp(&a.score))
        });
    }

    Some(Server {
        address: raw.address.to_string(),
        realname: format_name(hostname),
        map: format_name(
            raw.rules
                .get("mapname")
                .map(|v| v.as_slice())
                .unwrap_or(b""),
        ),
        geo,
        version: parsed.version,
        gamedir: rule_str(&raw.rules, "modname"),
        mode,
        modefull,
        mode2: parsed.mode2,
        impure: parsed.impure,
        slots: parsed.slots,
        numplayers,
        maxplayers,
        numbots,
        numspectators,
        enc,
        d0id,
        fballowed: parsed.fballowed,
        teamplay: parsed.teamplay,
        stats: parsed.stats,
        scoreinfo: parsed.scoreinfo,
        players,
    })
}

struct QcParsed {
    mode: String,
    version: String,
    impure: i32,
    slots: i32,
    fballowed: i32,
    teamplay: i32,
    stats: i32,
    mode2: String,
    player_order: i32,
    scoreinfo: ScoreInfo,
}

fn parse_qcstatus(raw: &str, _numplayers: i32) -> QcParsed {
    let mut qc = if raw.is_empty() {
        "?:::".to_string()
    } else {
        raw.to_string()
    };

    // Perl: if /^( [a-z?]+ ) ::: ( .+ )? $/ inject defaults
    if let Some(caps) = regex_short_qc(&qc) {
        qc = format!("{}:?:P9999:S0:F0:MUnknown::{}", caps.0, caps.1);
    }

    // Perl: s/:F[0-9]+:\KT[^:]+:/  — drop the T<?> field after flags, keep :F<n>:
    let re = FLAGS_T.get_or_init(|| regex::Regex::new(r"(:F[0-9]+:)T[^:]+:").unwrap());
    qc = re.replace(&qc, "$1").into_owned();

    let parts: Vec<&str> = qc.split(':').collect();
    let mode = parts.first().copied().unwrap_or("?").to_uppercase();
    let version = parts.get(1).copied().unwrap_or("?").to_string();
    let impure = skip_prefix(parts.get(2).copied().unwrap_or("P0"), 'P');
    let slots = skip_prefix(parts.get(3).copied().unwrap_or("S0"), 'S');
    let flags = skip_prefix(parts.get(4).copied().unwrap_or("F0"), 'F');
    let mode2_raw = parts.get(5).copied().unwrap_or("MUnknown");
    let mode2 = if mode2_raw == "MXonotic" {
        "VANILLA".to_string()
    } else if mode2_raw.len() > 1 {
        mode2_raw[1..].to_uppercase()
    } else {
        "UNKNOWN".to_string()
    };

    // split: mode ver impure slots flags mode2 [undef] pscoreinfo tscoreinfo tscores...
    // index 6 is the empty field from :: after mode2 (or T-stripped)
    let pscoreinfo = parts.get(7).copied();
    let tscoreinfo = parts.get(8).copied();
    let tscores = if parts.len() > 9 { &parts[9..] } else { &[] };

    let mut scoreinfo = ScoreInfo::default();
    let mut player_order = 0;

    if let Some(ps) = pscoreinfo {
        if let Some(si) = parse_scoreinfo_token(ps) {
            player_order = si.order;
            scoreinfo.player = si;
        }
    }

    if let Some(ts) = tscoreinfo {
        if let Some((pri, sec)) = parse_team_scoreinfo(ts) {
            if sec.label.is_empty() {
                scoreinfo.team.prefer = "pri".into();
                scoreinfo.team.pri = pri;
            } else {
                scoreinfo.team.prefer = "sec".into();
                scoreinfo.team.pri = pri;
                scoreinfo.team.sec = sec;
            }

            let mut it = tscores.iter();
            while let (Some(k), Some(v)) = (it.next(), it.next()) {
                let team_id = k.parse::<i32>().unwrap_or(0);
                let mapped = map_team(team_id);
                if scoreinfo.team.prefer == "sec" {
                    let mut sp = v.splitn(2, ',');
                    let pri_s = sp.next().unwrap_or("0").parse().unwrap_or(0);
                    let sec_s = sp.next().unwrap_or("0").parse().unwrap_or(0);
                    scoreinfo.team.pri.score.insert(mapped, pri_s);
                    scoreinfo.team.sec.score.insert(mapped, sec_s);
                } else {
                    scoreinfo
                        .team
                        .pri
                        .score
                        .insert(mapped, v.parse().unwrap_or(0));
                }
            }
        }
    }

    QcParsed {
        mode,
        version,
        impure,
        slots,
        fballowed: flags & 1,
        teamplay: if flags & 2 != 0 { 1 } else { 0 },
        stats: if flags & 4 != 0 { 1 } else { 0 },
        mode2,
        player_order,
        scoreinfo,
    }
}

fn regex_short_qc(qc: &str) -> Option<(String, String)> {
    // ^([a-z?]+):::(.+)?$
    let bytes = qc.as_bytes();
    let mut i = 0;
    while i < bytes.len() && (bytes[i].is_ascii_lowercase() || bytes[i] == b'?') {
        i += 1;
    }
    if i == 0 || !qc[i..].starts_with(":::") {
        return None;
    }
    let mode = qc[..i].to_string();
    let rest = qc[i + 3..].to_string();
    Some((mode, rest))
}

fn skip_prefix(s: &str, p: char) -> i32 {
    let t = s.strip_prefix(p).unwrap_or(s);
    t.parse().unwrap_or(0)
}

fn parse_scoreinfo_token(s: &str) -> Option<ScoreFlags> {
    // ^([a-z]+)([!<]+)?(?:,([a-z]+)([!<]+)?)?$  -- we only need the first pair here
    let (label, flags) = split_label_flags(s.split(',').next().unwrap_or(""))?;
    let order = if flags.contains('<') { 1 } else { 0 };
    Some(ScoreFlags {
        label,
        flags,
        order,
        score: BTreeMap::new(),
    })
}

fn parse_team_scoreinfo(s: &str) -> Option<(ScoreFlags, ScoreFlags)> {
    let mut bits = s.splitn(2, ',');
    let pri_raw = bits.next().unwrap_or("");
    let (label, flags) = split_label_flags(pri_raw)?;
    let pri = ScoreFlags {
        order: if flags.contains('<') { 1 } else { 0 },
        label,
        flags,
        score: BTreeMap::new(),
    };
    let sec = if let Some(sec_raw) = bits.next() {
        if let Some((label, flags)) = split_label_flags(sec_raw) {
            ScoreFlags {
                order: if flags.contains('<') { 1 } else { 0 },
                label,
                flags,
                score: BTreeMap::new(),
            }
        } else {
            ScoreFlags::default()
        }
    } else {
        ScoreFlags::default()
    };
    Some((pri, sec))
}

fn split_label_flags(s: &str) -> Option<(String, String)> {
    if s.is_empty() {
        return None;
    }
    let mut label_end = 0;
    for (i, c) in s.char_indices() {
        if c.is_ascii_lowercase() {
            label_end = i + c.len_utf8();
        } else {
            break;
        }
    }
    if label_end == 0 {
        return None;
    }
    let label = s[..label_end].to_string();
    let flags = s[label_end..].to_string();
    if !flags.chars().all(|c| c == '!' || c == '<') && !flags.is_empty() {
        // still accept; Perl is strict but we keep the label
    }
    Some((label, flags))
}

fn convert_player(p: &RawPlayer, qc: &QcParsed) -> Player {
    let name = format_name(&p.name);
    let mut team = map_team(p.team);
    let prio = if p.score == -666 || p.score == -616 || name == "unconnected" {
        team = 0;
        2
    } else if team == 0
        && ((p.score == 0 && is_time_label(&qc.scoreinfo.player.label)) || qc.teamplay != 0)
    {
        1
    } else {
        0
    };
    Player {
        name,
        score: p.score,
        ping: p.ping,
        team,
        prio,
    }
}

fn is_time_label(label: &str) -> bool {
    label == "fastest" || label == "time"
}

pub fn assemble_snapshot(servers: Vec<Server>, epoch: u64) -> Snapshot {
    let mut snap = Snapshot::default();
    snap.info.lastupdate_epoch = epoch;
    for s in servers {
        if s.numplayers > 0 {
            snap.info.activeservers += 1;
        }
        snap.info.totalservers += 1;
        snap.info.totalplayers += s.numplayers;
        snap.info.totalbots += s.numbots;
        snap.server.insert(s.address.clone(), s);
    }
    snap
}

// view helpers used by templates via precomputed fields
pub fn encryption_label(enc: i32) -> &'static str {
    match enc {
        3 => "Required",
        2 => "Requested",
        1 => "Supported",
        _ => "Not Supported",
    }
}

pub fn ucfirst(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

pub fn player_score_display(server: &Server, p: &Player) -> (String, Option<String>) {
    if p.score == -666 || (p.ping == 1 && p.team == 0) {
        return ("spec".into(), Some("spectator".into()));
    }
    if p.score == -616 {
        return ("oog".into(), Some("out of game".into()));
    }
    let label = &server.scoreinfo.player.label;
    if is_time_label(label) {
        return (score2time(p.score), None);
    }
    if server.mode == "LMS" {
        if p.score > 255 || p.score == 0 {
            return ("-".into(), None);
        }
        return (ordinate(p.score), None);
    }
    if server.mode == "COOP" && server.mode2 == "QUAKE" {
        return ((p.score * -1).to_string(), None);
    }
    (p.score.to_string(), None)
}

pub fn ping_kind(p: &Player) -> PingKind {
    if p.ping == 1 {
        PingKind::Arrows
    } else if p.ping == 0 {
        PingKind::Bot
    } else {
        PingKind::Number(p.ping)
    }
}

#[derive(Debug, Clone)]
pub enum PingKind {
    Arrows,
    Bot,
    Number(i32),
}

pub fn mode_tag(server: &Server) -> (String, bool) {
    if server.impure == 0 {
        ("OFFICIAL".into(), false)
    } else if server.mode2 != "VANILLA" {
        (server.mode2.clone(), server.mode2 == "INSTAGIB")
    } else {
        ("MODIFIED".into(), false)
    }
}

pub fn team_class(team: i32) -> Option<&'static str> {
    match team {
        1 => Some("team1red"),
        2 => Some("team2blue"),
        3 => Some("team3yellow"),
        4 => Some("team4pink"),
        _ => None,
    }
}

pub fn sorted_servers(snap: &Snapshot) -> Vec<&Server> {
    let mut v: Vec<&Server> = snap.server.values().collect();
    v.sort_by(|a, b| {
        b.numplayers
            .cmp(&a.numplayers)
            .then(b.realname.cmp(&a.realname))
    });
    v
}

pub fn trunc_with_title(s: &str, n: usize) -> (String, Option<String>) {
    let t = truncate_egc(s, n);
    if t.chars().count() < s.chars().count() {
        (t, Some(s.to_string()))
    } else {
        (t, None)
    }
}

use std::sync::OnceLock;
static FLAGS_T: OnceLock<regex::Regex> = OnceLock::new();

pub fn init_regex() {
    FLAGS_T.get_or_init(|| regex::Regex::new(r"(:F[0-9]+:)T[^:]+:").unwrap());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bans_parse() {
        let t = "B 1.2.3.4:\nX ignore\nB evil.host:\n";
        let b = parse_banned(t);
        assert!(b.contains(&"1.2.3.4".into()));
        assert!(b.contains(&"evil.host".into()));
    }

    #[test]
    fn qc_ctf() {
        init_regex();
        let q = parse_qcstatus("ctf:git:P56:S55:F7:MInstaGib::score!!:caps!!:5:7:14:4", 4);
        assert_eq!(q.mode, "CTF");
        assert_eq!(q.mode2, "INSTAGIB");
        assert_eq!(q.impure, 56);
        assert_eq!(q.slots, 55);
        assert_eq!(q.fballowed, 1);
        assert_eq!(q.teamplay, 1);
        assert_eq!(q.stats, 1);
        assert_eq!(q.scoreinfo.player.label, "score");
        assert_eq!(q.scoreinfo.team.prefer, "pri");
        assert_eq!(q.scoreinfo.team.pri.label, "caps");
        assert_eq!(q.scoreinfo.team.pri.score.get(&1).copied(), Some(7));
        assert_eq!(q.scoreinfo.team.pri.score.get(&2).copied(), Some(4));
    }
}
