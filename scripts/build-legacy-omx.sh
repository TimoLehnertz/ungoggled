#!/bin/sh
# Optional compatibility plugin for 32-bit Buster with /opt/vc userland.
# Dependencies: build-essential, pkg-config, curl, xz-utils,
# libgstreamer1.0-dev, libgstreamer-plugins-base1.0-dev, libraspberrypi-dev.
# No system libraries are replaced. Pass a writable absolute installation prefix.
set -eu
prefix=${1:?Usage: scripts/build-legacy-omx.sh /absolute/install/prefix}
case "$prefix" in /*) ;; *) echo 'Prefix must be absolute.' >&2; exit 1;; esac
test -f /opt/vc/include/IL/OMX_Broadcom.h
pkg-config --exists gstreamer-1.0 gstreamer-video-1.0 gstreamer-audio-1.0
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT HUP INT TERM
curl -fL --retry 2 https://gstreamer.freedesktop.org/src/gst-omx/gst-omx-1.14.4.tar.xz -o "$work/source.tar.xz"
printf '%s  %s\n' 969870e75c1f75c96f8783530e2c2932fc3afbfd976eb0c466f51dae268ea3d4 "$work/source.tar.xz" | sha256sum -c -
tar -xJf "$work/source.tar.xz" -C "$work"
# Configure has no --disable-gl switch. Hide only this optional dependency so
# the decoder does not create the legacy EGL renderer, which fails on FKMS.
cat > "$work/pkg-config-no-gl" <<'EOF'
#!/bin/sh
for arg do
    case "$arg" in *gstreamer-gl-1.0*) exit 1;; esac
done
exec pkg-config "$@"
EOF
chmod +x "$work/pkg-config-no-gl"
cd "$work/gst-omx-1.14.4"
PKG_CONFIG="$work/pkg-config-no-gl" ./configure \
    --prefix="$prefix" --with-omx-target=rpi \
    --with-omx-header-path=/opt/vc/include/IL \
    --disable-examples --disable-gtk-doc --disable-fatal-warnings
if grep -q '^#define HAVE_GST_GL ' config.h; then
    echo 'GL unexpectedly enabled; refusing incompatible build.' >&2
    exit 1
fi
make -j2
make install
printf 'Plugin built under %s/lib/gstreamer-1.0\n' "$prefix"
printf 'Run with GST_PLUGIN_PATH pointing there and --decoder omxh264dec.\n'
