#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
target=${1:-aarch64-unknown-linux-musl}
case "$target" in aarch64-unknown-linux-musl) arch=aarch64;; armv7-unknown-linux-musleabihf) arch=armv7l;; *) echo 'Unsupported Pi target' >&2;exit 1;; esac
cargo build --release --target "$target"
(cd web && npm ci --cache /tmp/dji-npm-cache --fetch-retries=0 && npm run build)
binary=target/$target/release/ungoggled
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
# Include UI and installer changes in the release identity, not only the binary.
digest=$(tar -cf - "$binary" web/dist scripts/install-release.sh scripts/prepare-pi.sh deploy/dji-hdmi.service | sha256sum | cut -c1-12)
name=ungoggled-$version-$arch
bundle=build/releases/$name
mkdir -p build/releases dist
# Start fresh so obsolete hashed UI assets cannot enter the new release.
if [[ -d "$bundle" ]]; then rm -rf -- "$bundle"; fi
mkdir -p "$bundle/bin" "$bundle/web"
install -m755 "$binary" "$bundle/bin/ungoggled"
cp -a web/dist/. "$bundle/web/"
install -m755 scripts/install-release.sh "$bundle/install.sh"
install -m755 scripts/prepare-pi.sh "$bundle/prepare-pi.sh"
cp deploy/dji-hdmi.service "$bundle/"
printf '%s-%s\n' "$version" "$digest" > "$bundle/VERSION"
printf '%s\n' "$arch" > "$bundle/ARCH"
cp README.md FINDINGS.MD THIRD_PARTY.md "$bundle/"
(cd "$bundle" && find bin web -type f -print0 | sort -z | xargs -0 sha256sum > SHA256SUMS && sha256sum install.sh prepare-pi.sh dji-hdmi.service VERSION ARCH README.md FINDINGS.MD THIRD_PARTY.md >> SHA256SUMS)
tar -czf "dist/$name.tar.gz" -C build/releases "$name"
sha256sum "dist/$name.tar.gz" > "dist/$name.tar.gz.sha256"
printf 'Release: dist/%s.tar.gz\n' "$name"
