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

The box is a platform: any service that binds `127.0.0.1:<port>` becomes a
subdomain in three moves.

1. DNS — `A` record for the subdomain → same VM IP
2. Caddy — append to `/etc/caddy/Caddyfile`, then `systemctl reload caddy`:

   ```text
   app.hub.example.com {
       reverse_proxy 127.0.0.1:8080
   }
   ```

3. Service — ship it as a systemd unit like [`deploy/sylver.service`](deploy/sylver.service),
   listening on loopback only so nothing bypasses TLS

Caddy issues each certificate automatically. Keep every service off the
public interfaces; the proxy is the single front door.

## Backups and disaster recovery

Everything stateful is two paths:

```text
/var/lib/sylvie/sylvie.db   users, devices, sessions, secret ciphertext
/var/lib/sylvie/files/      file blobs
```

The bootstrap installs a daily job (`/etc/cron.d/sylvie-backup`, 04:00)
snapshotting both into `/var/backups/sylvie/`, keeping 14 days. That protects
against a bad deploy — not against losing the box.

**Off-box copies are your responsibility.** Two supported ways:

1. From any machine: `deploy/pull-backup.sh root@hub.example.com` grabs a fresh
   snapshot into a dated local folder. Cron it from a machine that is
   reliably on.
2. Push to object storage (S3-compatible; Arvan works well for `.ir`) with
   rclone, wrapping the archive in `age` or `rclone crypt` first — the
   tarball contains plaintext files.

### Restoring onto a fresh box

```bash
curl -fsSL https://raw.githubusercontent.com/liagha/sylvie/main/deploy/bootstrap.sh | bash
systemctl stop sylver
cp db.snapshot /var/lib/sylvie/sylvie.db
tar xzf files.tgz -C /var/lib/sylvie
chown -R sylvie:sylvie /var/lib/sylvie
systemctl start sylver
```

Point DNS at the new box; Caddy re-issues certificates automatically.
Devices keep working — tokens live in the database you restored.

One asymmetry to respect: secret values decrypt only with your password
(derived vault key). A stolen backup is inert without it; a forgotten
password makes every backup of secrets unreadable. Files need no password.
