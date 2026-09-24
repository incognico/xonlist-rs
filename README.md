# xonlist

Rust rewrite of [incognico/xonlist](https://github.com/incognico/xonlist). The live site is <https://xonotic.lifeisabug.com>.

One binary queries the master servers, serves the HTML and JSON UI, records hourly activity, and renders the heatmap. nginx proxies to its Unix socket. There is no qstat, Perl, wget, or cron job.

It links the system `libssl` and `libsqlite3`. CSS, JavaScript, fonts, images, and HTML templates are compiled in, so those changes need a new build. Country flags need a GeoLite2 City database. That file is not in this repository; MaxMind does not allow redistribution.

`/server/…` pages send `noindex`. The main page does not.

| Data | Interval |
| --- | --- |
| Server list | 5 minutes |
| Ban list (`checkupdate.txt`) | 24 hours |
| Activity and heatmap | 30 minutes |

## Build

```bash
apt install pkg-config libssl-dev libsqlite3-dev
cargo build --release
```

The binary is `target/release/xonlist`. Release builds use `opt-level = "z"`, fat LTO, `panic = "abort"`, and `strip = true`.

Deploy steps, the systemd unit, and nginx are in [INSTALL.md](INSTALL.md).

## Run

```bash
./target/release/xonlist \
  --listen 127.0.0.1:8080 \
  --data-dir ./data \
  --geodb /usr/local/share/GeoIP2_k/GeoLite2-City.mmdb \
  --domain localhost:8080
```

Open `http://127.0.0.1:8080/`. The first server-list query runs in the background.

Every flag also reads an `XONLIST_*` variable (`XONLIST_LISTEN`, `XONLIST_DATA_DIR`, `XONLIST_GEODB`, `XONLIST_DOMAIN`, `XONLIST_MASTERS`, …). Command-line values win. `--masters` is a comma-separated list of `host` or `host:port`. The default list is `master1.xonotic.org:42863`, `master2.xonotic.org:27950`, `master3.xonotic.org:27950`, `master4.xonotic.org:42863`, `dpmaster.deathmask.net`, `dpmaster.tchr.no`, `dpm.dpmaster.org:27777`, `dpm4.xonotic.xyz:27777`, and `dpm6.xonotic.xyz:27777`. IPv4 is tried before IPv6.
