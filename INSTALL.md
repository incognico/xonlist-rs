# Install

xonlist is one binary. It queries the master servers, serves the HTML and JSON UI, records hourly activity, and renders the heatmap. nginx stays in front and proxies to a Unix socket. There is no qstat, Perl, wget, or cron job.

CSS, JavaScript, fonts, images, and HTML templates are compiled into the binary. A change to those files needs a new build.

The paths below are for the live host of <https://xonotic.lifeisabug.com> (Debian, nginx, user `www-data`, site tree `/home/www/xonotic.lifeisabug.com`). The unit shipped in `etc/systemd/system/xonlist.service` still points at `/srv/www` and `User=http`. Use the unit in this document on that host.

## What you need

On the build machine, a Rust toolchain. Build on the same architecture as the server, or copy a release binary built for it.

On the server at runtime:

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

To keep heatmap history, copy the Perl app's database before the first start:

```bash
install -o www-data -g www-data -m 644 \
  /home/www/xonotic.lifeisabug.com/app/files/activity.db \
  /home/www/xonotic.lifeisabug.com/data/activity.db
```

The table layout matches. An empty database is created if that file is absent.

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

The Perl app and this binary both want the unit name `xonlist.service` and the socket `/run/xonlist/xonlist.socket`. Stop and disable the Perl unit before enabling this one.

`/etc/systemd/system/xonlist.service`:

```ini
[Unit]
Description=Xonotic Server List
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=www-data
Group=www-data
RuntimeDirectory=xonlist
RuntimeDirectoryMode=0750
WorkingDirectory=/home/www/xonotic.lifeisabug.com
ExecStart=/home/www/xonotic.lifeisabug.com/xonlist \
  --socket /run/xonlist/xonlist.socket \
  --data-dir /home/www/xonotic.lifeisabug.com/data \
  --geodb /usr/local/share/GeoIP2_k/GeoLite2-City.mmdb \
  --domain xonotic.lifeisabug.com
Restart=on-failure
RestartSec=3
NoNewPrivileges=yes

[Install]
WantedBy=multi-user.target
```

```bash
systemctl daemon-reload
systemctl enable --now xonlist.service
```

The process removes a stale socket, binds `/run/xonlist/xonlist.socket`, and sets it to mode `0660`. nginx has to run as `www-data` (or share that group) to connect. `RuntimeDirectory=xonlist` creates `/run/xonlist` for the service user.

## nginx

If the vhost already proxies the whole site to `unix:/run/xonlist/xonlist.socket`, leave that proxy in place. The binary serves `/`, `/server/…`, `/servers`, `/endpoint/json`, `/heatmap.png`, and the embedded files under `css/`, `js/`, `images/`, and `fonts/`.

A `location` that still aliases the Perl tree `app/public/` would keep serving the old CSS and JavaScript. Point every location at the socket:

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

`--socket` wins over `--listen`. `--domain` is the public site name used in links (`https://xonotic.lifeisabug.com`, or `http://localhost:8080` when the value starts with `localhost` or `127.`). TTL values are seconds. `--masters` is a comma-separated list of `host` or `host:port`.
