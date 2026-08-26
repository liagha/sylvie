#!/usr/bin/env bash
set -euo pipefail

# pull a fresh snapshot of the hub onto this machine
# usage: ./pull-backup.sh root@your-box [destination]

HOST=${1:?usage: pull-backup.sh user@host [destination]}
DEST=${2:-./sylvie-snapshots}
STAMP=$(date +%Y-%m-%d-%H%M)

mkdir -p "$DEST/$STAMP"
ssh -o BatchMode=yes "$HOST" 'sqlite3 /var/lib/sylvie/sylvie.db ".backup /tmp/db.snapshot" && tar czf /tmp/files.tgz -C /var/lib/sylvie files'
scp -q -o BatchMode=yes "$HOST:/tmp/db.snapshot" "$DEST/$STAMP/"
scp -q -o BatchMode=yes "$HOST:/tmp/files.tgz" "$DEST/$STAMP/"
ssh -o BatchMode=yes "$HOST" 'rm -f /tmp/db.snapshot /tmp/files.tgz'

echo "snapshot: $DEST/$STAMP"
ls -la "$DEST/$STAMP"

# restore recipe (any machine, any provider):
#   bootstrap a new box per docs/deploy.md, then as root:
#   systemctl stop sylver
#   cp db.snapshot /var/lib/sylvie/sylvie.db
#   tar xzf files.tgz -C /var/lib/sylvie
#   chown -R sylvie:sylvie /var/lib/sylvie && systemctl start sylver
