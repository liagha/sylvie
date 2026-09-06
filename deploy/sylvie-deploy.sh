#!/usr/bin/env bash
set -euo pipefail

STAGE=/var/lib/sylvie/staging
BIN=/usr/local/bin
WEB=/var/lib/sylvie/web

rm -rf "$STAGE"
install -d -o sylvie -g sylvie -m 700 "$STAGE"
tar xzf - -C "$STAGE"

SYLVIE_BIND_ADDR=127.0.0.1:17400 \
SYLVIE_DB_PATH="$STAGE/boot.db" \
SYLVIE_STORAGE_PATH="$STAGE/files" \
SYLVIE_WEB_DIR="$STAGE/web" \
"$STAGE/sylver" &
PID=$!
sleep 2
if kill -0 "$PID" 2>/dev/null; then
    RESULT=ok
else
    RESULT=dead
fi
kill "$PID" 2>/dev/null || true
wait "$PID" 2>/dev/null || true
test "$RESULT" = ok

install -m 755 "$STAGE/sylver" "$BIN/sylver"
install -m 755 "$STAGE/sylvie" "$BIN/sylvie"
install -o sylvie -g sylvie -m 644 "$STAGE"/web/* "$WEB/"

systemctl restart sylver
sleep 1
systemctl is-active --quiet sylver
rm -rf "$STAGE"
echo deployed