#!/usr/bin/env bash
# Builds the WebAssembly game into ./dist (served as-is by GitHub Pages).
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release --target wasm32-unknown-unknown
REG=$(cargo metadata --format-version 1 | python3 -c 'import json,sys; m=json.load(sys.stdin); print("\n".join(p["manifest_path"] for p in m["packages"] if p["name"] in ("miniquad","quad-snd")))')
rm -rf dist && mkdir -p dist
for manifest in $REG; do
  dir=$(dirname "$manifest")
  case "$dir" in *miniquad*) cp "$dir/js/gl.js" dist/ ;; *quad-snd*) cp "$dir/js/audio.js" dist/ ;; esac
done
cp web/index.html web/driftmoor.js dist/
cp target/wasm32-unknown-unknown/release/driftmoor.wasm dist/
touch dist/.nojekyll
ls -la dist
