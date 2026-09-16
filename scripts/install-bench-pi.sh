#!/bin/sh
# Migrate the supplied Cosmostreamer Buster Pi, preserving its Wi-Fi credentials.
# Run from the project with built assets, a matching receiver and legacy plugin.
# Changes next boot only; the current development service/network stay running.
set -eu
[ "$(id -u)" -eq 0 ] || { echo 'Run as root.' >&2; exit 1; }
binary=${1:?Usage: scripts/install-bench-pi.sh binary libgstomx.so source.tar.xz}
plugin=${2:?Missing legacy OMX plugin}
source_archive=${3:?Missing corresponding gst-omx source archive}
test "$(uname -m)" = armv7l
test -f /opt/Cosmostreamer-NG/run.sh
test -d /sys/class/net/wifi0
test -s /run/cosmostreamer/hostapd.conf
test -f web/dist/index.html
test -s "$plugin"
test -s "$source_archive"
"$binary" --version
mount -o remount,rw /
trap 'sync; mount -o remount,ro /' EXIT
backup=/var/lib/dji-hdmi/previous-boot
mkdir -p "$backup" /etc/dji-hdmi /etc/default /etc/systemd/system.conf.d
for file in /etc/rc.local /etc/dhcpcd.conf; do
    test -f "$backup/$(basename "$file")" || cp -p "$file" "$backup/"
done
install -Dm755 "$binary" /usr/local/bin/dji-hdmi
install -Dm755 scripts/prepare-pi.sh /usr/local/lib/dji-hdmi/prepare-pi.sh
install -Dm755 scripts/bench-ap.sh /usr/local/lib/dji-hdmi/bench-ap.sh
install -Dm644 "$plugin" /usr/local/lib/dji-hdmi/plugins/libgstomx.so
mkdir -p /usr/local/share/dji-hdmi/web /usr/local/share/dji-hdmi/source
cp -R web/dist/. /usr/local/share/dji-hdmi/web/
cp "$source_archive" /usr/local/share/dji-hdmi/source/gst-omx-1.14.4.tar.xz
cp README.md FINDINGS.MD THIRD_PARTY.md /usr/local/share/dji-hdmi/
cp scripts/build-legacy-omx.sh /usr/local/share/dji-hdmi/source/
install -m600 /run/cosmostreamer/hostapd.conf /etc/dji-hdmi/hostapd.conf
cat > /etc/default/dji-hdmi <<'EOF'
DJI_HDMI_DECODER=omxh264dec
GST_PLUGIN_PATH=/usr/local/lib/dji-hdmi/plugins
GST_OMX_CONFIG_DIR=/etc/xdg
EOF
install -m644 deploy/dji-hdmi.service /etc/systemd/system/dji-hdmi.service
install -m644 deploy/bench-ap.service /etc/systemd/system/dji-hdmi-ap.service
if ! grep -qx 'denyinterfaces wifi0' /etc/dhcpcd.conf; then
    printf '\n# Onboard Wi-Fi belongs to dji-hdmi-ap.service.\ndenyinterfaces wifi0\n' >> /etc/dhcpcd.conf
fi
# PID 1 replaces the old application watchdog after reboot. Leave its current
# owner alone: killing an armed hardware watchdog can reset the running Pi.
cat > /etc/systemd/system.conf.d/dji-hdmi-watchdog.conf <<'EOF'
[Manager]
RuntimeWatchdogSec=10s
EOF
cat > /etc/rc.local <<'EOF'
#!/bin/sh
# Media and the Wi-Fi AP now start through systemd.
exit 0
EOF
chmod +x /etc/rc.local
systemctl daemon-reload
systemctl enable dji-hdmi dji-hdmi-ap dnsmasq
echo 'Installed for next boot. Current test processes remain running.'
echo "Previous boot configuration saved in $backup."
