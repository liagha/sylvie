#!/bin/sh
# Build the browser vault: compile sylvie-web to wasm and stage the dashboard
# assets next to the server. Run after `cargo build` so the native crates are
# ready; the result lands in ./web and is served at /assets.
set -e

OUT="web"

cargo build -p sylvie-web --target wasm32-unknown-unknown --release

wasm-bindgen \
    target/wasm32-unknown-unknown/release/sylvie_web.wasm \
    --out-dir "$OUT" \
    --target web \
    --out-name sylvie_web

cp crates/server/web/shell.js "$OUT/shell.js"

echo "web assets written to $OUT/"
