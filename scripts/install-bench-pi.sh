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
ap_config=/run/cosmostreamer/hostapd.conf
if [ ! -s "$ap_config" ]; then
    ap_config=/etc/dji-hdmi/hostapd.conf
fi
test -s "$ap_config"
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
release=/opt/dji-hdmi/releases/$("$binary" --version | awk '{print $2}')-$(sha256sum "$binary" | cut -c1-12)
mkdir -p "$release/bin" "$release/web"
install -m755 "$binary" "$release/bin/dji-hdmi"
cp -R web/dist/. "$release/web/"
ln -sfn "$release" /opt/dji-hdmi/current
install -Dm755 scripts/prepare-pi.sh /usr/local/lib/dji-hdmi/prepare-pi.sh
install -Dm755 scripts/bench-ap.sh /usr/local/lib/dji-hdmi/bench-ap.sh
install -Dm644 "$plugin" /usr/local/lib/dji-hdmi/plugins/libgstomx.so
mkdir -p /usr/local/share/dji-hdmi/web /usr/local/share/dji-hdmi/source
cp -R web/dist/. /usr/local/share/dji-hdmi/web/
cp "$source_archive" /usr/local/share/dji-hdmi/source/gst-omx-1.14.4.tar.xz
cp README.md FINDINGS.MD THIRD_PARTY.md /usr/local/share/dji-hdmi/
cp scripts/build-legacy-omx.sh /usr/local/share/dji-hdmi/source/
if [ "$ap_config" != /etc/dji-hdmi/hostapd.conf ]; then
    install -m600 "$ap_config" /etc/dji-hdmi/hostapd.conf
fi
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
# This image also starts a proprietary framebuffer splash outside rc.local.
# It must not retain a display layer alongside the independent HDMI receiver.
if [ -f /etc/systemd/system/splashscreen.service ] &&
    grep -q '/opt/Cosmostreamer-NG/' /etc/systemd/system/splashscreen.service; then
    systemctl disable --now splashscreen.service
fi
systemctl daemon-reload
systemctl enable dji-hdmi dji-hdmi-ap dnsmasq
echo 'Installed for next boot. Current test processes remain running.'
echo "Previous boot configuration saved in $backup."
