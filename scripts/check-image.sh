#!/bin/bash
# Validation inside the image chroot with /dev, /proc and /sys mounted.
set -euo pipefail
systemd-analyze verify /etc/systemd/system/dji-hdmi.service /etc/systemd/system/dji-hdmi-firstboot.service /etc/systemd/system/ungoggled-update-recovery.service
test -x /usr/local/lib/dji-hdmi/update-helper
for unit in ungoggled-update-recovery dji-hdmi dji-hdmi-firstboot NetworkManager ssh; do systemctl is-enabled "$unit"; done
[[ -u /usr/bin/sudo && $(stat -c %a /tmp) == 1777 ]]
[[ $(stat -c '%u:%g' /usr/bin/sudo) == 0:0 ]]
[[ $(id -u root) == 0 ]]
! id dji >/dev/null 2>&1
[[ ! -s /etc/machine-id ]]
! compgen -G '/etc/ssh/ssh_host_*' >/dev/null
profile=/etc/NetworkManager/system-connections/dji-hdmi.nmconnection
[[ $(stat -c %a "$profile") == 600 ]]
nmcli --offline connection modify connection.id dji-hdmi < "$profile" > /tmp/dji-checked.nmconnection
python3 - <<'PY'
import configparser
import ctypes
from pathlib import Path

profile = configparser.ConfigParser(interpolation=None)
profile.read('/tmp/dji-checked.nmconnection')
state = configparser.ConfigParser()
state.read('/var/lib/NetworkManager/NetworkManager.state')
assert state.getboolean('main', 'WirelessEnabled'), 'Wi-Fi disabled in NetworkManager'
assert Path('/etc/modprobe.d/rfkill_default.conf').read_text().strip() == 'options rfkill default_state=1'
assert profile['wifi']['ssid'] == 'ungoggled'
assert profile['wifi-security']['psk'] == 'ungoggled'
root = next(line.split(':') for line in Path('/etc/shadow').read_text().splitlines()
            if line.startswith('root:'))
crypt = ctypes.CDLL('libcrypt.so.1').crypt
crypt.argtypes = (ctypes.c_char_p, ctypes.c_char_p)
crypt.restype = ctypes.c_char_p
assert crypt(b'ungoggled', root[1].encode()) == root[1].encode(), 'Root password mismatch'
PY
mkdir -p /run/sshd
ssh-keygen -q -t ed25519 -N '' -f /tmp/dji-check-hostkey
sshd -T -h /tmp/dji-check-hostkey -C user=root,host=localhost,addr=127.0.0.1 > /tmp/dji-check-sshd
grep -qx 'permitrootlogin yes' /tmp/dji-check-sshd
grep -qx 'passwordauthentication yes' /tmp/dji-check-sshd
for plugin in h264parse capssetter kmssink fpsdisplaysink jpegenc videorate videoscale videoconvert video4linux2; do
    gst-inspect-1.0 "$plugin" >/dev/null
done
/opt/dji-hdmi/current/bin/ungoggled serve --no-autostart --output none \
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
echo 'PASS: image services, permissions, default Wi-Fi/root credentials, SSH login policy, media plugins and ARM64 HTTP server'
