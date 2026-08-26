#!/usr/bin/env bash
set -euo pipefail

# install the sylvie CLI from the latest release
# usage: curl -fsSL https://raw.githubusercontent.com/liagha/sylvie/main/deploy/install-client.sh | bash

BIN_DIR=${SYLVIE_PREFIX:-$HOME/.local/bin}
OS=$(uname -s)
ARCH=$(uname -m)

fallback() {
    echo "no prebuilt binary for $OS/$ARCH — building with cargo"
    command -v cargo >/dev/null || { echo "install rust first: https://rustup.rs" >&2; exit 1; }
    cargo install --git https://github.com/liagha/sylvie --bin sylvie
    exit 0
}

case "$OS/$ARCH" in
    Linux/x86_64) ASSET="sylvie-x86_64-linux" ;;
    *) fallback ;;
esac

mkdir -p "$BIN_DIR"
URL="https://github.com/liagha/sylvie/releases/latest/download/$ASSET"
curl -fL --retry 3 -o "$BIN_DIR/sylvie" "$URL" || fallback
chmod 755 "$BIN_DIR/sylvie"

case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) echo "note: add $BIN_DIR to your PATH" ;;
esac
"$BIN_DIR/sylvie" --version
echo "next: sylvie register --url https://your-hub.example.com --user <you>"
