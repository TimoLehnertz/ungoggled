#!/bin/sh
# Run on the Pi from an unpacked project with a matching release binary.
set -eu
[ "$(id -u)" -eq 0 ] || { echo 'Run as root.' >&2; exit 1; }
binary=${1:?Usage: sudo scripts/install-pi.sh /path/to/dji-hdmi}
test -f web/dist/index.html
test -x "$binary"
"$binary" --version
install -Dm755 "$binary" /usr/local/bin/dji-hdmi
install -Dm755 scripts/prepare-pi.sh /usr/local/lib/dji-hdmi/prepare-pi.sh
mkdir -p /usr/local/share/dji-hdmi/web
cp -R web/dist/. /usr/local/share/dji-hdmi/web/
install -Dm644 deploy/dji-hdmi.service /etc/systemd/system/dji-hdmi.service
systemctl daemon-reload
systemctl enable --now dji-hdmi.service

