# xonlist

Self-contained [Xonotic](https://www.xonotic.org/) gameserver list. Queries
dpmaster instances over UDP, serves the HTML/JSON UI, tracks hourly activity and
renders the heatmap — all from one binary. No qstat, Perl, wget or cron.

This is a Rust rewrite of [incognico/xonlist](https://github.com/incognico/xonlist)
(https://xonotic.lifeisabug.com).

Cached data is only refreshed when its TTL expires:

| Data | TTL |
| --- | --- |
| Server list | 5 minutes |
| Ban list (`checkupdate.txt`) | 24 hours |
| Activity + heatmap | 30 minutes |

## Build

```bash
cargo build --release
```

The binary is `target/release/xonlist`. Static assets and HTML templates are compiled in. The only optional runtime file is a MaxMind GeoLite2 MMDB for country flags.

## Run

TCP (development):

```bash
./target/release/xonlist \
  --listen 127.0.0.1:8080 \
  --data-dir ./data \
  --geodb /path/to/GeoLite2-City.mmdb \
  --domain localhost:8080
```

Unix socket (production, matches the bundled systemd unit):

```bash
./xonlist \
  --socket /run/xonlist/xonlist.socket \
  --data-dir /srv/www/xonotic.lifeisabug.com/data \
  --geodb /home/k/GeoLite2-City.mmdb
```

Install `etc/systemd/system/xonlist.service` and place the binary at
`/srv/www/xonotic.lifeisabug.com/xonlist`.

## Config

All flags also accept `XONLIST_*` environment variables (`XONLIST_SOCKET`,
`XONLIST_DATA_DIR`, `XONLIST_GEODB`, `XONLIST_DOMAIN`, `XONLIST_SERVER_TTL`, …).
