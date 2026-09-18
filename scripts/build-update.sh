#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
python3 scripts/version.py --check
cargo build --locked --release --target aarch64-unknown-linux-musl
(cd web && npm ci --cache /tmp/dji-npm-cache --fetch-retries=0 && npm run build)
python3 scripts/package-update.py
cargo build --locked
version=$(python3 scripts/version.py)
target/debug/ungoggled check-update --file "dist/ungoggled-$version.update.tar.gz"
