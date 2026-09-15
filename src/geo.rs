use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use maxminddb::geoip2;
use parking_lot::RwLock;
use tracing::{info, warn};

use crate::model::Snapshot;

struct Loaded {
    mtime_secs: u64,
    reader: maxminddb::Reader<Vec<u8>>,
}

/// Live GeoIP from a manually maintained MMDB. Reloads when the file mtime changes.
pub struct Geo {
    path: PathBuf,
    loaded: RwLock<Option<Loaded>>,
}

impl Geo {
    pub fn open(path: PathBuf) -> Self {
        let geo = Self {
            path,
            loaded: RwLock::new(None),
        };
        geo.reload_if_changed();
        geo
    }

    pub fn reload_if_changed(&self) {
        let mtime = std::fs::metadata(&self.path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs());
        let Some(mtime) = mtime else {
            let mut g = self.loaded.write();
            if g.is_some() {
                warn!(path = %self.path.display(), "GeoIP database disappeared");
                *g = None;
            }
            return;
        };
        if self
            .loaded
            .read()
            .as_ref()
            .is_some_and(|l| l.mtime_secs == mtime)
        {
            return;
        }
        match maxminddb::Reader::open_readfile(&self.path) {
            Ok(reader) => {
                info!(path = %self.path.display(), "GeoIP database loaded");
                *self.loaded.write() = Some(Loaded {
                    mtime_secs: mtime,
                    reader,
                });
            }
            Err(e) => {
                warn!(
                    error = %e,
                    path = %self.path.display(),
                    "GeoIP database not loaded; countries will be ??"
                );
            }
        }
    }

    pub fn country_for_address(&self, address: &str) -> String {
        let Some(ip) = ip_from_address(address) else {
            return "??".into();
        };
        let g = self.loaded.read();
        let Some(loaded) = g.as_ref() else {
            return "??".into();
        };
        match loaded.reader.lookup::<geoip2::City>(ip) {
            Ok(Some(city)) => city
                .country
                .and_then(|c| c.iso_code)
                .unwrap_or("??")
                .to_string(),
            _ => "??".into(),
        }
    }

    pub fn apply(&self, snap: &mut Snapshot) {
        self.reload_if_changed();
        for s in snap.server.values_mut() {
            s.geo = self.country_for_address(&s.address);
        }
    }
}

fn ip_from_address(address: &str) -> Option<IpAddr> {
    address.parse::<SocketAddr>().ok().map(|a| a.ip())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_v4() {
        assert_eq!(
            ip_from_address("1.2.3.4:26000").unwrap(),
            "1.2.3.4".parse::<IpAddr>().unwrap()
        );
    }
}
