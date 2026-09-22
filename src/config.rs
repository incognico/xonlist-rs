use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    pub listen: SocketAddr,
    pub socket: Option<PathBuf>,
    pub data_dir: PathBuf,
    pub geodb: PathBuf,
    pub domain: String,
    pub title: String,
    pub desc: String,
    pub masters: Vec<String>,
    pub bans_url: String,
    pub server_ttl: Duration,
    pub bans_ttl: Duration,
    pub activity_interval: Duration,
    pub retries: u32,
    pub query_timeout_ms: u64,
}

impl Config {
    pub fn parse() -> Self {
        let mut cfg = Self::defaults();
        for (key, val) in env::vars() {
            if let Some(name) = env_name(&key) {
                if let Err(e) = cfg.set(name, &val) {
                    die(&e);
                }
            }
        }
        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            if arg == "--help" || arg == "-h" {
                print_help();
                std::process::exit(0);
            }
            let Some(rest) = arg.strip_prefix("--") else {
                die(&format!("unknown argument {arg}"));
            };
            let (name, val) = if let Some((name, val)) = rest.split_once('=') {
                (name, val.to_string())
            } else {
                let Some(val) = args.next() else {
                    die(&format!("missing value for --{rest}"));
                };
                (rest, val)
            };
            if let Err(e) = cfg.set(name, &val) {
                die(&e);
            }
        }
        cfg
    }

    fn defaults() -> Self {
        Self {
            listen: "127.0.0.1:8080".parse().unwrap(),
            socket: None,
            data_dir: PathBuf::from("data"),
            geodb: PathBuf::from("/home/k/GeoLite2-City.mmdb"),
            domain: "xonotic.lifeisabug.com".into(),
            title: "XonList - Xonotic Server List".into(),
            desc: "XonList - Gameserver list for Xonotic. Servers, players, scores & more - Find Xonotic servers to play on.".into(),
            masters: split_masters(
                "master1.xonotic.org:42863,dpmaster.deathmask.net,dpmaster.tchr.no,dpm.dpmaster.org:27777",
            ),
            bans_url: "https://gitlab.com/xonotic/xonotic/raw/master/misc/infrastructure/checkupdate.txt".into(),
            server_ttl: Duration::from_secs(300),
            bans_ttl: Duration::from_secs(86400),
            activity_interval: Duration::from_secs(1800),
            retries: 5,
            query_timeout_ms: 800,
        }
    }

    fn set(&mut self, name: &str, val: &str) -> Result<(), String> {
        match name {
            "listen" => self.listen = parse_listen(val)?,
            "socket" => {
                self.socket = if val.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(val))
                }
            }
            "data-dir" => self.data_dir = PathBuf::from(val),
            "geodb" => self.geodb = PathBuf::from(val),
            "domain" => self.domain = val.to_string(),
            "title" => self.title = val.to_string(),
            "desc" => self.desc = val.to_string(),
            "masters" => self.masters = split_masters(val),
            "bans-url" => self.bans_url = val.to_string(),
            "server-ttl" => self.server_ttl = parse_secs(val)?,
            "bans-ttl" => self.bans_ttl = parse_secs(val)?,
            "activity-interval" => self.activity_interval = parse_secs(val)?,
            "retries" => {
                self.retries = val
                    .parse()
                    .map_err(|_| format!("retries: expected an integer, got {val}"))?
            }
            "query-timeout-ms" => {
                self.query_timeout_ms = val
                    .parse()
                    .map_err(|_| format!("query-timeout-ms: expected an integer, got {val}"))?
            }
            _ => return Err(format!("unknown option --{name}")),
        }
        Ok(())
    }

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

fn env_name(key: &str) -> Option<&'static str> {
    Some(match key {
        "XONLIST_LISTEN" => "listen",
        "XONLIST_SOCKET" => "socket",
        "XONLIST_DATA_DIR" => "data-dir",
        "XONLIST_GEODB" => "geodb",
        "XONLIST_DOMAIN" => "domain",
        "XONLIST_TITLE" => "title",
        "XONLIST_DESC" => "desc",
        "XONLIST_MASTERS" => "masters",
        "XONLIST_BANS_URL" => "bans-url",
        "XONLIST_SERVER_TTL" => "server-ttl",
        "XONLIST_BANS_TTL" => "bans-ttl",
        "XONLIST_ACTIVITY_INTERVAL" => "activity-interval",
        "XONLIST_RETRIES" => "retries",
        "XONLIST_QUERY_TIMEOUT_MS" => "query-timeout-ms",
        _ => return None,
    })
}

fn split_masters(val: &str) -> Vec<String> {
    val.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn parse_listen(val: &str) -> Result<SocketAddr, String> {
    val.parse()
        .map_err(|_| format!("listen: expected host:port, got {val}"))
}

fn parse_secs(val: &str) -> Result<Duration, String> {
    let secs: u64 = val
        .parse()
        .map_err(|_| format!("expected seconds, got {val}"))?;
    Ok(Duration::from_secs(secs))
}

fn die(msg: &str) -> ! {
    eprintln!("xonlist: {msg}");
    eprintln!("try --help");
    std::process::exit(2);
}

fn print_help() {
    println!(
        "\
xonlist — Xonotic gameserver list

  --listen ADDR            TCP listen address (default 127.0.0.1:8080)
  --socket PATH            Unix socket; used instead of --listen
  --data-dir DIR           cache, activity db, heatmap (default data)
  --geodb PATH             GeoLite2 MMDB
  --domain NAME            public site name
  --title TEXT
  --desc TEXT
  --masters LIST           comma-separated host[:port]
  --bans-url URL           checkupdate.txt
  --server-ttl SECS
  --bans-ttl SECS
  --activity-interval SECS
  --retries N
  --query-timeout-ms MS

Each flag also reads XONLIST_* (XONLIST_LISTEN, XONLIST_DATA_DIR, …).
Command-line values win over the environment."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masters_and_ttl() {
        let mut cfg = Config::defaults();
        cfg.set("masters", "a.example, b.example:27950").unwrap();
        cfg.set("server-ttl", "90").unwrap();
        cfg.set("retries", "2").unwrap();
        assert_eq!(
            cfg.masters,
            vec!["a.example".to_string(), "b.example:27950".to_string()]
        );
        assert_eq!(cfg.server_ttl, Duration::from_secs(90));
        assert_eq!(cfg.retries, 2);
    }
}
