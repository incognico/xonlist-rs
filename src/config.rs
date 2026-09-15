use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;

#[derive(Debug, Clone, Parser)]
#[command(name = "xonlist", about = "Xonotic gameserver list")]
pub struct Config {
    /// TCP listen address (used when --socket is not set)
    #[arg(long, env = "XONLIST_LISTEN", default_value = "127.0.0.1:8080")]
    pub listen: SocketAddr,

    /// Unix socket path (takes precedence over --listen)
    #[arg(long, env = "XONLIST_SOCKET")]
    pub socket: Option<PathBuf>,

    /// Directory for cache, activity db and heatmap
    #[arg(long, env = "XONLIST_DATA_DIR", default_value = "data")]
    pub data_dir: PathBuf,

    /// MaxMind GeoLite2 City/Country MMDB
    #[arg(
        long,
        env = "XONLIST_GEODB",
        default_value = "/home/k/GeoLite2-City.mmdb"
    )]
    pub geodb: PathBuf,

    #[arg(long, env = "XONLIST_DOMAIN", default_value = "xonotic.lifeisabug.com")]
    pub domain: String,

    #[arg(
        long,
        env = "XONLIST_TITLE",
        default_value = "XonList - Xonotic Server List"
    )]
    pub title: String,

    #[arg(
        long,
        env = "XONLIST_DESC",
        default_value = "XonList - Gameserver list for Xonotic. Servers, players, scores & more - Find Xonotic servers to play on."
    )]
    pub desc: String,

    /// Master servers, comma-separated host[:port]
    #[arg(
        long,
        env = "XONLIST_MASTERS",
        default_value = "dpmaster.deathmask.net,dpmaster.tchr.no,dpm.dpmaster.org:27777",
        value_delimiter = ','
    )]
    pub masters: Vec<String>,

    /// Ban list URL (checkupdate.txt)
    #[arg(
        long,
        env = "XONLIST_BANS_URL",
        default_value = "https://gitlab.com/xonotic/xonotic/raw/master/misc/infrastructure/checkupdate.txt"
    )]
    pub bans_url: String,

    /// How long a server-list query stays valid
    #[arg(long, env = "XONLIST_SERVER_TTL", default_value = "300", value_parser = parse_secs)]
    pub server_ttl: Duration,

    /// How long the downloaded ban list stays valid
    #[arg(long, env = "XONLIST_BANS_TTL", default_value = "86400", value_parser = parse_secs)]
    pub bans_ttl: Duration,

    /// Activity / heatmap interval
    #[arg(long, env = "XONLIST_ACTIVITY_INTERVAL", default_value = "1800", value_parser = parse_secs)]
    pub activity_interval: Duration,

    /// UDP retries per game server (qstat -retry 5)
    #[arg(long, env = "XONLIST_RETRIES", default_value_t = 5)]
    pub retries: u32,

    /// Per-attempt UDP timeout in milliseconds
    #[arg(long, env = "XONLIST_QUERY_TIMEOUT_MS", default_value_t = 800)]
    pub query_timeout_ms: u64,
}

fn parse_secs(s: &str) -> Result<Duration, std::num::ParseIntError> {
    Ok(Duration::from_secs(s.parse()?))
}

impl Config {
    pub fn snapshot_path(&self) -> PathBuf {
        self.data_dir.join("snapshot.json")
    }

    pub fn bans_path(&self) -> PathBuf {
        self.data_dir.join("checkupdate.txt")
    }

    pub fn activity_path(&self) -> PathBuf {
        self.data_dir.join("activity.db")
    }

    pub fn heatmap_path(&self) -> PathBuf {
        self.data_dir.join("heatmap.png")
    }

    pub fn asset_base(&self) -> String {
        let d = &self.domain;
        if d.starts_with("localhost") || d.starts_with("127.") || d.starts_with('[') {
            format!("http://{d}")
        } else {
            format!("https://{d}")
        }
    }
}
