#!/bin/bash
# Validation inside the image chroot with /dev, /proc and /sys mounted.
set -euo pipefail
systemd-analyze verify /etc/systemd/system/dji-hdmi.service /etc/systemd/system/dji-hdmi-firstboot.service
for unit in dji-hdmi dji-hdmi-firstboot NetworkManager ssh; do systemctl is-enabled "$unit"; done
[[ -u /usr/bin/sudo && $(stat -c %a /tmp) == 1777 ]]
[[ $(stat -c '%u:%g' /usr/bin/sudo) == 0:0 ]]
id dji >/dev/null
[[ ! -s /etc/machine-id ]]
! compgen -G '/etc/ssh/ssh_host_*' >/dev/null
profile=/etc/NetworkManager/system-connections/dji-hdmi.nmconnection
[[ $(stat -c %a "$profile") == 600 ]]
nmcli --offline connection modify connection.id dji-hdmi < "$profile" > /tmp/dji-checked.nmconnection
for plugin in h264parse kmssink fpsdisplaysink jpegenc videorate videoscale videoconvert video4linux2; do
    gst-inspect-1.0 "$plugin" >/dev/null
done
/opt/dji-hdmi/current/bin/dji-hdmi serve --no-autostart --output none \
    --listen 127.0.0.1:18190 --runtime-dir /tmp/dji-check-run \
    --data-dir /tmp/dji-check-data --web-dir /opt/dji-hdmi/current/web >/tmp/dji-check.log 2>&1 &
server=$!
trap 'kill "$server" 2>/dev/null || true; wait "$server" 2>/dev/null || true' EXIT
for _ in {1..30}; do
    if curl -fsS http://127.0.0.1:18190/api/settings >/tmp/dji-check-settings.json; then break; fi
    sleep .2
done
curl -fsS http://127.0.0.1:18190/api/images/no-signal.png -o /tmp/dji-check.png
curl -fsS http://127.0.0.1:18190/ >/dev/null
python3 - <<'PY'
import json
from pathlib import Path
assert json.loads(Path('/tmp/dji-check-settings.json').read_text())['fallback_image'] == 'no-signal.png'
assert Path('/tmp/dji-check.png').read_bytes().startswith(b'\x89PNG\r\n\x1a\n')
PY
echo 'PASS: image services, permissions, Wi-Fi profile, media plugins and ARM64 HTTP server'
