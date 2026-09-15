use std::net::IpAddr;
use std::path::Path;

use maxminddb::geoip2;
use tracing::warn;

pub struct Geo {
    reader: maxminddb::Reader<Vec<u8>>,
}

impl Geo {
    pub fn open(path: &Path) -> Option<Self> {
        match maxminddb::Reader::open_readfile(path) {
            Ok(reader) => Some(Self { reader }),
            Err(e) => {
                warn!(error = %e, path = %path.display(), "GeoIP database not loaded; countries will be ??");
                None
            }
        }
    }

    pub fn country(&self, ip: IpAddr) -> String {
        match self.reader.lookup::<geoip2::City>(ip) {
            Ok(Some(city)) => city
                .country
                .and_then(|c| c.iso_code)
                .unwrap_or("??")
                .to_string(),
            Ok(None) => "??".into(),
            Err(_) => "??".into(),
        }
    }
}

pub fn lookup(geo: Option<&Geo>, ip: IpAddr) -> String {
    geo.map(|g| g.country(ip)).unwrap_or_else(|| "??".into())
}
