# Deployment

Sylvie is one static binary behind a TLS proxy. This guide takes a fresh
Debian/Ubuntu box to `https://hub.example.com` in about five minutes, and leaves
room for the rest of your hub (each future service gets its own subdomain
block in the Caddyfile).

## 0. Prerequisites

- an x86_64 VM you can SSH into as root — Debian 12 or Ubuntu 22.04+,
  1 GB RAM is plenty. Local providers give better latency for `.ir`;
  international hosts work too but some restrict Iranian signups.
  (ARM boxes work too, but you build the binaries yourself — see
  docs/clients.md.)
- DNS: an `A` record for `hub.example.com` → the VM's IPv4 address (and `AAAA`
  if you want v6). Caddy obtains certificates automatically once DNS and
  port 80/443 reach the box.

## 1. Bootstrap

```bash
ssh root@YOUR_BOX
curl -fsSL https://raw.githubusercontent.com/liagha/sylvie/main/deploy/bootstrap.sh | bash
```

What it does: installs Caddy, creates a locked `sylvie` system user,
downloads release binaries for your architecture into `/usr/local/bin`,
writes `/etc/sylvie/sylvie.env`, enables `sylver.service` +
`caddy.service`, and locks the firewall to SSH + 80/443.

The server binds `127.0.0.1:7400` only — nothing reaches it except Caddy.

## 2. Verify

```bash
dig +short hub.example.com                 # your box's IP
curl -s https://hub.example.com/api/v1/me  # {"error":"unauthorized"}
systemctl status sylver caddy
```

## 3. First device

From any machine with the CLI:

```bash
sylvie register --url https://hub.example.com --user <you>
```

That account is now permanent; every other device uses `sylvie login`.

## Layout on disk

```text
/usr/local/bin/{sylvie,sylver}          binaries
/etc/sylvie/sylvie.env                  server configuration
/var/lib/sylvie/sylvie.db               database (users, secrets ciphertext)
/var/lib/sylvie/files/                  file blobs
```

## Updating

```bash
SYLVIE_VERSION=vX.Y.Z bash -c 'set -e
  curl -fL -o /usr/local/bin/sylver \
    https://github.com/liagha/sylvie/releases/download/$SYLVIE_VERSION/sylver-x86_64-linux
  curl -fL -o /usr/local/bin/sylv \
    https://github.com/liagha/sylvie/releases/download/$SYLVIE_VERSION/sylvie-x86_64-linux
  systemctl restart sylver'
```

Migrations run automatically at startup. Back up before major jumps.

## Adding hub services later

Copy the block in [`deploy/Caddyfile`](deploy/Caddyfile): one subdomain, one
`reverse_proxy` line, its own systemd unit bound to `127.0.0.1:<port>`.
Caddy issues certificates per subdomain automatically — just add the DNS
record first.

## Backups

Everything stateful is two paths:

```bash
sqlite3 /var/lib/sylvie/sylvie.db ".backup /tmp/sylvie.db.bak"
tar czf sylvie-backup.tgz /var/lib/sylvie/sylvie.db.bak /var/lib/sylvie/files
```

Take both together; the DB alone cannot restore file contents and vice
versa. Restore = stop service, replace both, start service.
