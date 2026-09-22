# Install

xonlist is one binary. It queries the master servers, serves the HTML and JSON UI, records hourly activity, and renders the heatmap. nginx stays in front and proxies to a Unix socket. There is no qstat, Perl, wget, or cron job.

CSS, JavaScript, fonts, images, and HTML templates are compiled into the binary. A change to those files needs a new build.

The paths below are the live host of <https://xonotic.lifeisabug.com>: Debian, nginx, user `www-data`, site tree `/home/www/xonotic.lifeisabug.com`. The unit in `etc/systemd/system/xonlist.service` is the one that host runs.

## What you need

On the build machine, a Rust toolchain and the libraries the binary links against. `pkg-config` finds them. On Debian:

```bash
apt install pkg-config libssl-dev libsqlite3-dev
```

Build on the same architecture as the server, or copy a release binary built for it.

On the server at runtime the binary links `libssl.so.3`, `libcrypto.so.3`, and `libsqlite3.so.0`. nginx already uses that OpenSSL.

- The binary `target/release/xonlist`.
- A data directory the service user can write. The process creates `snapshot.json`, `checkupdate.txt`, `activity.db`, and `heatmap.png` there.
- Optional: a MaxMind GeoLite2 City database. On this host it is `/usr/local/share/GeoIP2_k/GeoLite2-City.mmdb`. It is not in this repository; MaxMind does not allow redistribution. Without it the list still runs and country flags stay empty.
- Outbound UDP to the master servers and to the game servers, and outbound HTTPS for the ban list (`checkupdate.txt` on GitLab).

Refresh intervals baked into the binary:

| Data | Interval |
| --- | --- |
| Server list | 5 minutes |
| Ban list | 24 hours |
| Activity and heatmap | 30 minutes |

## Build

```bash
cargo build --release
```

The binary is `target/release/xonlist`.

## Lay out the files

```bash
install -o www-data -g www-data -m 755 target/release/xonlist \
  /home/www/xonotic.lifeisabug.com/xonlist
install -d -o www-data -g www-data -m 755 \
  /home/www/xonotic.lifeisabug.com/data
```

The live `data/activity.db` was copied from the old Perl tree at `app/files/activity.db`. The table layout matches. An empty database is created if that file is absent. The Perl tree is still on disk and is not used by this service.

## Try it on TCP first

This does not touch the live socket:

```bash
./xonlist \
  --listen 127.0.0.1:8080 \
  --data-dir ./data \
  --geodb /usr/local/share/GeoIP2_k/GeoLite2-City.mmdb \
  --domain localhost:8080
```

Open `http://127.0.0.1:8080/`. The first server-list query runs in the background and can take a few seconds.

## systemd

Install `etc/systemd/system/xonlist.service` as `/etc/systemd/system/xonlist.service`. It listens on `/run/xonlist/xonlist.socket`.

```bash
systemctl daemon-reload
systemctl enable --now xonlist.service
```

The process removes a stale socket, binds `/run/xonlist/xonlist.socket`, and sets it to mode `0660`. nginx has to run as `www-data` (or share that group) to connect. `RuntimeDirectory=xonlist` creates `/run/xonlist` for the service user.

## nginx

The live vhost proxies the whole site to `unix:/run/xonlist/xonlist.socket`, except `/.well-known/acme-challenge/`. The binary serves `/`, `/server/…`, `/servers`, `/endpoint/json`, `/heatmap.png`, and the embedded files under `css/`, `js/`, `images/`, and `fonts/`. `/server/…` responses include `X-Robots-Tag: noindex`.

Keep every content location on that socket:

```nginx
location / {
    proxy_pass http://unix:/run/xonlist/xonlist.socket:;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
}
```

```bash
nginx -t && systemctl reload nginx
```

Asset URLs are not hashed. After a binary update, reload the page with a hard refresh.

TLS, the HTTP-to-HTTPS redirect, and HSTS stay in the existing vhost. This app does not terminate TLS.

## Update

Rebuild, replace the binary, and restart the unit. Static files change only when the binary changes.

```bash
cargo build --release
install -o www-data -g www-data -m 755 target/release/xonlist \
  /home/www/xonotic.lifeisabug.com/xonlist
systemctl restart xonlist.service
```

The data directory is not part of the binary. Leave `data/` in place across updates.

## Configuration

Every flag has an `XONLIST_*` environment variable: `XONLIST_SOCKET`, `XONLIST_LISTEN`, `XONLIST_DATA_DIR`, `XONLIST_GEODB`, `XONLIST_DOMAIN`, `XONLIST_TITLE`, `XONLIST_DESC`, `XONLIST_MASTERS`, `XONLIST_BANS_URL`, `XONLIST_SERVER_TTL`, `XONLIST_BANS_TTL`, `XONLIST_ACTIVITY_INTERVAL`, `XONLIST_RETRIES`, `XONLIST_QUERY_TIMEOUT_MS`.

`--socket` wins over `--listen`. `--domain` is the public site name used in links (`https://xonotic.lifeisabug.com`, or `http://localhost:8080` when the value starts with `localhost` or `127.`). TTL values are seconds. `--masters` is a comma-separated list of `host` or `host:port`. The default masters are `master1.xonotic.org:42863`, `dpmaster.deathmask.net`, `dpmaster.tchr.no`, and `dpm.dpmaster.org:27777`. Each master is contacted over IPv4 before IPv6. `--geodb` defaults to `/usr/local/share/GeoIP2_k/GeoLite2-City.mmdb`.
