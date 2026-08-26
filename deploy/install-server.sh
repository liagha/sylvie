#!/usr/bin/env bash
set -euo pipefail

# install the sylver hub server binary from the latest release
# usage: curl -fsSL https://raw.githubusercontent.com/liagha/sylvie/main/deploy/install-server.sh | bash
# full setup (systemd, caddy, firewall): deploy/bootstrap.sh

if [[ $(uname -s)/$(uname -m) != Linux/x86_64 ]]; then
    echo "prebuilt server ships for Linux x86_64 only; see docs/clients.md for building" >&2
    exit 1
fi

BIN_DIR=${SYLVIE_PREFIX:-/usr/local/bin}
if [[ $BIN_DIR == /usr/* && ${EUID:-$(id -u)} -ne 0 ]]; then
    echo "run as root, or set SYLVIE_PREFIX to a writable directory" >&2
    exit 1
fi

mkdir -p "$BIN_DIR"
URL="https://github.com/liagha/sylvie/releases/latest/download/sylver-x86_64-linux"
curl -fL --retry 3 -o "$BIN_DIR/sylver" "$URL"
chmod 755 "$BIN_DIR/sylver"

"$BIN_DIR/sylver" &
PID=$!
sleep 1
kill "$PID" 2>/dev/null || true
wait "$PID" 2>/dev/null || true

cat <<'NEXT'

installed. to run it as a service on a public box:
  curl -fsSL https://raw.githubusercontent.com/liagha/sylvie/main/deploy/bootstrap.sh | bash
or by hand:
  useradd -r -d /var/lib/sylvie sylvie
  edit /etc/systemd/system/sylver.service from the repo's deploy/ directory

NEXT
