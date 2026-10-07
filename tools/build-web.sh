#!/usr/bin/env bash
# Build the browser version of CadForge (egui on WebGL) into dist-web/.
#   tools/build-web.sh          optimized release build
#   tools/build-web.sh dev      faster, less optimized build for iterating
set -euo pipefail
cd "$(dirname "$0")/.."
profile=release
dir=release
if [ "${1:-}" = "dev" ]; then profile=web; dir=web; fi
cargo build -p cadforge-app --lib --target wasm32-unknown-unknown --profile "$profile"
rm -rf dist-web && mkdir -p dist-web
wasm-bindgen --target web --no-typescript --out-dir dist-web "target/wasm32-unknown-unknown/$dir/cadforge_app.wasm"
cp crates/cadforge-app/web/index.html dist-web/index.html
ls -la dist-web
