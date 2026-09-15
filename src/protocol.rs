//! DarkPlaces / dpmaster UDP query (replaces qstat).

use std::collections::{HashMap, HashSet};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use tokio::net::UdpSocket;
use tokio::time::timeout;
use tracing::{debug, warn};

const HEADER: &[u8] = &[0xFF, 0xFF, 0xFF, 0xFF];
const GETSERVERS: &[u8] = b"\xff\xff\xff\xffgetservers Xonotic 3 empty full\n";
const GETSTATUS: &[u8] = b"\xff\xff\xff\xffgetstatus\n";
const MASTER_PORT: u16 = 27950;

#[derive(Debug, Clone)]
pub struct RawPlayer {
    pub score: i32,
    pub ping: i32,
    pub team: i32,
    pub name: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct RawStatus {
    pub address: SocketAddr,
    pub rules: HashMap<String, Vec<u8>>,
    pub players: Vec<RawPlayer>,
}

pub async fn query_all_masters(masters: &[String]) -> HashSet<SocketAddr> {
    let mut tasks = Vec::new();
    for m in masters {
        let m = m.clone();
        tasks.push(tokio::spawn(async move { query_master(&m).await }));
    }
    let mut out = HashSet::new();
    for t in tasks {
        match t.await {
            Ok(Ok(list)) => out.extend(list),
            Ok(Err(e)) => warn!(error = %e, "master query failed"),
            Err(e) => warn!(error = %e, "master task join failed"),
        }
    }
    out
}

pub async fn query_master(hostport: &str) -> anyhow::Result<Vec<SocketAddr>> {
    let addr = resolve_master(hostport).await?;
    let sock = UdpSocket::bind("0.0.0.0:0").await?;
    sock.connect(addr).await?;
    sock.send(GETSERVERS).await?;

    let mut servers = Vec::new();
    let deadline = Duration::from_secs(3);
    let start = tokio::time::Instant::now();
    let mut buf = vec![0u8; 65535];
    loop {
        let remaining = deadline.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            break;
        }
        match timeout(remaining, sock.recv(&mut buf)).await {
            Ok(Ok(n)) => {
                let (found, eot) = parse_getservers_response(&buf[..n]);
                servers.extend(found);
                if eot {
                    break;
                }
            }
            Ok(Err(e)) => {
                debug!(error = %e, hostport, "master recv error");
                break;
            }
            Err(_) => break,
        }
    }
    Ok(servers)
}

async fn resolve_master(hostport: &str) -> anyhow::Result<SocketAddr> {
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) if p.chars().all(|c| c.is_ascii_digit()) => {
            (h.to_string(), p.parse::<u16>().unwrap_or(MASTER_PORT))
        }
        _ => (hostport.to_string(), MASTER_PORT),
    };
    let mut addrs = tokio::net::lookup_host((host.as_str(), port)).await?;
    addrs
        .next()
        .ok_or_else(|| anyhow::anyhow!("no addresses for {hostport}"))
}

pub fn parse_getservers_response(buf: &[u8]) -> (Vec<SocketAddr>, bool) {
    let mut data = buf;
    if data.starts_with(HEADER) {
        data = &data[4..];
    }
    if let Some(i) = data.iter().position(|&b| b == b'\\' || b == b'/') {
        data = &data[i..];
    }
    let mut out = Vec::new();
    let mut eot = false;
    let mut i = 0;
    while i < data.len() {
        match data[i] {
            b'\\' => {
                i += 1;
                if i + 6 <= data.len() && data[i..].starts_with(b"EOT") {
                    eot = true;
                    break;
                }
                if i + 6 > data.len() {
                    break;
                }
                let ip = Ipv4Addr::new(data[i], data[i + 1], data[i + 2], data[i + 3]);
                let port = u16::from_be_bytes([data[i + 4], data[i + 5]]);
                i += 6;
                if port != 0 && !ip.is_unspecified() && !ip.is_broadcast() {
                    out.push(SocketAddr::from((ip, port)));
                }
            }
            b'/' => {
                i += 1;
                if i + 18 > data.len() {
                    break;
                }
                let mut octets = [0u8; 16];
                octets.copy_from_slice(&data[i..i + 16]);
                let port = u16::from_be_bytes([data[i + 16], data[i + 17]]);
                i += 18;
                if port != 0 {
                    out.push(SocketAddr::from((Ipv6Addr::from(octets), port)));
                }
            }
            _ => i += 1,
        }
    }
    (out, eot)
}

pub async fn query_servers(
    addrs: impl IntoIterator<Item = SocketAddr>,
    retries: u32,
    timeout_ms: u64,
) -> Vec<RawStatus> {
    let attempt_to = Duration::from_millis(timeout_ms);
    let futs = addrs.into_iter().map(|addr| async move {
        for _ in 0..retries.max(1) {
            match timeout(attempt_to, query_server_once(addr)).await {
                Ok(Ok(Some(st))) => return Some(st),
                Ok(Ok(None)) => continue,
                Ok(Err(_)) => continue,
                Err(_) => continue,
            }
        }
        None
    });
    futures::future::join_all(futs)
        .await
        .into_iter()
        .flatten()
        .collect()
}

async fn query_server_once(addr: SocketAddr) -> std::io::Result<Option<RawStatus>> {
    let bind: SocketAddr = if addr.is_ipv6() {
        "[::]:0".parse().unwrap()
    } else {
        "0.0.0.0:0".parse().unwrap()
    };
    let sock = UdpSocket::bind(bind).await?;
    sock.connect(addr).await?;
    sock.send(GETSTATUS).await?;
    let mut buf = vec![0u8; 65535];
    let n = sock.recv(&mut buf).await?;
    Ok(parse_status_response(addr, &buf[..n]))
}

pub fn parse_status_response(address: SocketAddr, buf: &[u8]) -> Option<RawStatus> {
    let mut data = buf;
    if data.starts_with(HEADER) {
        data = &data[4..];
    }
    let nl = data.iter().position(|&b| b == b'\n')?;
    let cmd = &data[..nl];
    if !cmd.starts_with(b"statusResponse") {
        return None;
    }
    data = &data[nl + 1..];
    let info_end = data.iter().position(|&b| b == b'\n').unwrap_or(data.len());
    let rules = parse_infostring(&data[..info_end]);
    let mut players = Vec::new();
    if info_end < data.len() {
        for line in data[info_end + 1..].split(|&b| b == b'\n') {
            if line.is_empty() {
                continue;
            }
            if let Some(p) = parse_player_line(line) {
                players.push(p);
            }
        }
    }
    Some(RawStatus {
        address,
        rules,
        players,
    })
}

fn parse_infostring(buf: &[u8]) -> HashMap<String, Vec<u8>> {
    let mut map = HashMap::new();
    if buf.is_empty() {
        return map;
    }
    let s = if buf[0] == b'\\' { &buf[1..] } else { buf };
    let parts: Vec<&[u8]> = s.split(|&b| b == b'\\').collect();
    let mut i = 0;
    while i + 1 < parts.len() {
        let k = String::from_utf8_lossy(parts[i]).into_owned();
        map.insert(k, parts[i + 1].to_vec());
        i += 2;
    }
    map
}

fn parse_player_line(line: &[u8]) -> Option<RawPlayer> {
    let line = trim(line);
    let (score, rest) = parse_score(line)?;
    let rest = trim_start(rest);
    let (ping, rest) = parse_i32(rest)?;
    let rest = trim_start(rest);
    let (team, rest) = if rest.first() == Some(&b'"') {
        (0, rest)
    } else {
        let (t, r) = parse_i32(rest)?;
        (t, trim_start(r))
    };
    let name = parse_quoted(rest)?;
    Some(RawPlayer {
        score,
        ping,
        team,
        name,
    })
}

fn parse_score(buf: &[u8]) -> Option<(i32, &[u8])> {
    let (tok, rest) = take_token(buf)?;
    if tok.contains(&b'.') {
        let f: f32 = std::str::from_utf8(tok).ok()?.parse().ok()?;
        Some(((f * 100.0) as i32, rest))
    } else {
        let n: i32 = std::str::from_utf8(tok).ok()?.parse().ok()?;
        Some((n, rest))
    }
}

fn parse_i32(buf: &[u8]) -> Option<(i32, &[u8])> {
    let (tok, rest) = take_token(buf)?;
    let n: i32 = std::str::from_utf8(tok).ok()?.parse().ok()?;
    Some((n, rest))
}

fn take_token(buf: &[u8]) -> Option<(&[u8], &[u8])> {
    if buf.is_empty() {
        return None;
    }
    let end = buf
        .iter()
        .position(|&b| b.is_ascii_whitespace())
        .unwrap_or(buf.len());
    if end == 0 {
        return None;
    }
    Some((&buf[..end], &buf[end..]))
}

fn parse_quoted(buf: &[u8]) -> Option<Vec<u8>> {
    let buf = trim_start(buf);
    if buf.first() != Some(&b'"') {
        return if buf.is_empty() {
            Some(Vec::new())
        } else {
            Some(buf.to_vec())
        };
    }
    let inner = &buf[1..];
    if let Some(end) = inner.iter().position(|&b| b == b'"') {
        Some(inner[..end].to_vec())
    } else {
        Some(inner.to_vec())
    }
}

fn trim(buf: &[u8]) -> &[u8] {
    trim_start(trim_end(buf))
}

fn trim_start(buf: &[u8]) -> &[u8] {
    let i = buf
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(buf.len());
    &buf[i..]
}

fn trim_end(buf: &[u8]) -> &[u8] {
    let i = buf
        .iter()
        .rposition(|b| !b.is_ascii_whitespace())
        .map(|i| i + 1)
        .unwrap_or(0);
    &buf[..i]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn master_ipv4() {
        let mut pkt = Vec::from(&b"\xff\xff\xff\xffgetserversResponse"[..]);
        pkt.push(b'\\');
        pkt.extend_from_slice(&[1, 2, 3, 4, 0x65, 0x90]);
        pkt.push(b'\\');
        pkt.extend_from_slice(b"EOT\0\0\0");
        let (addrs, eot) = parse_getservers_response(&pkt);
        assert!(eot);
        assert_eq!(addrs.len(), 1);
        assert_eq!(addrs[0], "1.2.3.4:26000".parse().unwrap());
    }

    #[test]
    fn player_int_and_float() {
        let p = parse_player_line(br#"10 50 5 "Player""#).unwrap();
        assert_eq!(p.score, 10);
        assert_eq!(p.ping, 50);
        assert_eq!(p.team, 5);
        assert_eq!(p.name, b"Player");

        let p = parse_player_line(br#"1.5 20 14 "CA""#).unwrap();
        assert_eq!(p.score, 150);
        assert_eq!(p.team, 14);
    }

    #[test]
    fn status_roundtrip() {
        let body = b"\xff\xff\xff\xffstatusResponse\n\\hostname\\^1Test\\mapname\\dance\\clients\\1\\bots\\0\\sv_maxclients\\16\\gameversion\\22\\qcstatus\\dm:git:P0:S15:F0:MXonotic::score!!\n10 40 0 \"^2foo\"\n";
        let st = parse_status_response("1.2.3.4:26000".parse().unwrap(), body).unwrap();
        assert_eq!(st.rules.get("mapname").unwrap(), b"dance");
        assert_eq!(st.players.len(), 1);
        assert_eq!(st.players[0].score, 10);
    }
}
