#!/bin/bash
# Source checkout installation, using a release binary already built for this Pi.
set -euo pipefail
cd "$(dirname "$0")/.."
binary=${1:?Usage: sudo scripts/install-pi.sh /path/to/pi-binary}
[[ $(id -u) == 0 ]] || { echo 'Run with sudo.' >&2;exit 1; }
[[ $(uname -m) == aarch64 && $(getconf LONG_BIT) == 64 ]] || { echo 'Use 64-bit Raspberry Pi OS on a Pi 4.' >&2; exit 1; }
test -s web/dist/index.html
"$binary" --version
bundle=$(mktemp -d)
trap 'rm -rf -- "$bundle"' EXIT
mkdir -p "$bundle/bin" "$bundle/web"
install -m755 "$binary" "$bundle/bin/ungoggled"
cp -a web/dist/. "$bundle/web/"
install -m755 scripts/install-release.sh "$bundle/install.sh"
install -m755 scripts/prepare-pi.sh "$bundle/prepare-pi.sh"
cp deploy/dji-hdmi.service deploy/ungoggled-update-recovery.service "$bundle/"
version=$("$binary" --version | awk '{print $2}')
digest=$(tar -cf - "$binary" web/dist scripts/install-release.sh deploy/dji-hdmi.service | sha256sum | cut -c1-12)
printf '%s-%s\n' "$version" "$digest" > "$bundle/VERSION"
uname -m > "$bundle/ARCH"
(cd "$bundle" && find . -type f ! -name SHA256SUMS -print0 | sort -z | xargs -0 sha256sum > SHA256SUMS && ./install.sh)
