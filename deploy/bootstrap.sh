#!/usr/bin/env bash
set -euo pipefail

# bootstrap a fresh Debian/Ubuntu box as the hub.example.com hub host
# usage: curl -fsSL <url>/bootstrap.sh | bash   (or: bash bootstrap.sh)

if [[ ${EUID:-$(id -u)} -ne 0 ]]; then
    echo "run as root" >&2
    exit 1
fi

VERSION=${SYLVIE_VERSION:-v0.2.0}
BASE_URL="https://github.com/liagha/sylvie/releases/download/${VERSION}"

if [[ $(uname -m) != x86_64 ]]; then
    echo "prebuilt binaries ship for x86_64 only."
    echo "on other architectures install Rust and: cargo install --git https://github.com/liagha/sylvie"
    echo "see docs/clients.md" >&2
    exit 1
fi
ARCH="x86_64-linux"

echo "== packages"
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq ca-certificates curl ufw debian-keyring debian-archive-keyring apt-transport-https >/dev/null

echo "== caddy"
if ! command -v caddy >/dev/null; then
    install -d /usr/share/keyrings
    curl -fsSL "https://dl.cloudsmith.io/public/caddy/stable/gpg.key" | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
    curl -fsSL "https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt" -o /etc/apt/sources.list.d/caddy-stable.list
    apt-get update -qq
    apt-get install -y -qq caddy >/dev/null
fi

echo "== sylvie user and dirs"
id -u sylvie >/dev/null 2>&1 || useradd --system --home /var/lib/sylvie --shell /usr/sbin/nologin sylvie
install -d -o sylvie -g sylvie -m 700 /var/lib/sylvie/files

echo "== binaries (${ARCH})"
curl -fL --retry 3 -o /usr/local/bin/sylvie-server "${BASE_URL}/sylvie-server-${ARCH}"
curl -fL --retry 3 -o /usr/local/bin/sylvie "${BASE_URL}/sylvie-${ARCH}"
chmod 755 /usr/local/bin/sylvie-server /usr/local/bin/sylvie

echo "== config"
install -d -m 755 /etc/sylvie
cat > /etc/sylvie/sylvie.env <<'ENV'
SYLVIE_BIND_ADDR=127.0.0.1:7400
SYLVIE_DB_PATH=/var/lib/sylvie/sylvie.db
SYLVIE_STORAGE_PATH=/var/lib/sylvie/files
SYLVIE_LOG_LEVEL=info
SYLVIE_MAX_FILE_SIZE=1073741824
SYLVIE_AUTH_ATTEMPTS=5
SYLVIE_AUTH_WINDOW_SECS=300
ENV
chmod 640 /etc/sylvie/sylvie.env && chown root:sylvie /etc/sylvie/sylvie.env

echo "== caddy config"
cat > /etc/caddy/Caddyfile <<'CADDY'
hub.example.com {
	reverse_proxy 127.0.0.1:7400
	header {
		Strict-Transport-Security "max-age=31536000; include-subdomains"
		X-Content-Type-Options "nosniff"
		Referrer-Policy "no-referrer"
		-Server
	}
	request_body {
		max_size 1GB
	}
}
CADDY

echo "== services and firewall"
systemctl daemon-reload
systemctl enable --now sylvie-server.service
systemctl enable --now caddy.service
ufw allow openssh >/dev/null 2>&1 || true
ufw allow 80,443/tcp >/dev/null 2>&1 || true
yes | ufw enable >/dev/null 2>&1 || true

sleep 1
systemctl is-active --quiet sylvie-server && echo "sylvie-server: running"
systemctl is-active --quiet caddy && echo "caddy: running"
echo
echo "done. once hub.example.com points at this machine:"
echo "  curl https://hub.example.com/api/v1/me"
echo "  sylvie register --url https://hub.example.com --user <you>"
