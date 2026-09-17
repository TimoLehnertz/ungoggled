# ungoggled

Turn a Raspberry Pi 4 into an HDMI receiver for **DJI Goggles 3**, with a local
web interface for setup and monitoring.

Goggles 3 with both **O3 Air Unit** and **O4 Air Unit Pro** have been tested with
live HDMI output. Use the Pi's USB-C port for the goggles and power the Pi
separately.

## Features

- Hardware-decoded video over HDMI, with automatic 1080p mode selection.
- Configurable HDMI resolution and refresh rate from the display's supported modes.
- A selectable fallback image when video is lost or the receiver is stopped.
- Browser preview at up to **640 × 360, 5 fps**, independently switchable.
- Incoming resolution, measured camera fps, HDMI refresh rate and rendered fps.
- Raspberry Pi temperature and a **30-minute history**, sampled once per second
  and held in RAM.
- Wi-Fi access-point SSID and password controls.
- Automatic startup and recovery after USB disconnects or stream changes.
- Application updates without reflashing the SD card.

## Hardware and wiring

You need a Raspberry Pi 4, microSD card of at least 8 GB, a USB-C data cable,
a micro-HDMI cable, Goggles 3, and a paired O3 or O4 Pro air unit.

1. Power the Pi separately through a suitable GPIO supply or PoE arrangement.
2. Connect **Goggles USB-C → Pi USB-C**. The Pi's rectangular USB-A ports do not
   provide the USB role used by this receiver.
3. Connect your display to **HDMI0**, nearest the Pi's USB-C port.
4. Confirm a live camera image inside the goggles.

For clean video, turn **Settings → Camera → Advanced Camera Settings → Camera
View Recording → OFF** in the goggles. Enable it to include their overlays.
The receiver cannot remove overlays already embedded in the incoming image.

## Install the SD-card image

The image is based on **Raspberry Pi OS Lite 64-bit, Debian 13 Trixie**.

1. Download `ungoggled-0.2.0-pi4-arm64.img.xz` from the release files.
2. Write it to the microSD card with one of the [recommended
   writers](#recommended-writers). Pass the `.img.xz` in directly; these tools
   decompress while writing, so do not unpack it first.
3. Skip any OS customization the tool offers: the image already contains its
   network and login setup.
4. Verify the card, insert it into the Pi, and power on.
5. Join **ungoggled** using the password **ungoggled**.
6. Open **http://192.168.50.1:8090**.

### Recommended writers

The image expands to about 6.5 GiB, so the card must be at least 8 GB. Prefer a
tool that verifies the write afterwards; silent write failures on worn or
counterfeit cards are the most common cause of a Pi that never boots.

| Tool | Install | Notes |
| --- | --- | --- |
| **Caligula** | Arch: `pacman -S caligula`; Nix: `nixpkgs#caligula`; or a [prebuilt binary](https://github.com/ifd3f/caligula/releases) | Terminal UI. Lists only removable devices, decompresses `.img.xz` inline, and hash-verifies the card afterwards. |
| **Impression** | Arch: `pacman -S impression`; or [Flathub](https://flathub.org/apps/io.gitlab.adhami3310.Impression) | GTK interface. Writes through udisks2, so only the write is privileged. |
| **Raspberry Pi Imager** | packaged by most distributions as `rpi-imager` | Select **Use custom** and choose the `.img.xz`. See the Wayland caveat below. |

On Wayland desktops, prefer Caligula or Impression. Raspberry Pi Imager
re-executes its entire Qt GUI as root over X11, which fails on compositors that
do not authorize root X11 clients: under Hyprland it exits immediately with
`Authorization required, but no authorization protocol specified`. Working
around it needs `xhost +si:localuser:root` before every launch.

Any tool that writes a raw image works too. With `dd`, decompress on the fly and
write to the whole device, never a partition:

```bash
xzcat ungoggled-0.2.0-pi4-arm64.img.xz \
  | sudo dd of=/dev/sdX bs=4M conv=fsync oflag=direct status=progress
sync
```

Confirm the target with `lsblk` immediately beforehand; `dd` does not check
whether it is a removable device and does not verify the result.

### Default credentials

| Setting | Default |
| --- | --- |
| Wi-Fi SSID | `ungoggled` |
| Wi-Fi password | `ungoggled` |
| SSH username | `root` |
| SSH password | `ungoggled` |
| Web interface | `http://192.168.50.1:8090` |

Connect with `ssh root@192.168.50.1`. Change Wi-Fi credentials in the web
interface and the root password with `passwd` if desired. Application updates
preserve existing passwords; these defaults apply to freshly flashed images.

The root partition expands on first boot, and each device generates its own
SSH host keys. The image's Wi-Fi radio country is
**DE**; configure the correct country if using it elsewhere.

> The new Trixie image requires a physical boot/decoder check on a Pi. The earlier
> O3/O4 HDMI confirmations were on the supplied Buster installation. See
> [validation notes](FINDINGS.MD) for the current evidence and remaining checks.

## Use the web interface

**Start/stop** controls USB reception. **Reconnect goggles** restarts the USB
session. The display controller stays running so it can show the fallback image.

**Fallback image:** upload PNG, JPEG or WebP (up to 12 MiB), then select a
thumbnail. Images are converted to PNG, limited to 1920 × 1080 and displayed with
preserved aspect ratio. Up to 16 images can be stored. The selection survives
reboots; a “No signal” image is included.

**HDMI mode:** automatic mode prefers progressive 1080p at up to 60 Hz. You can
choose another advertised mode up to 1080p60. Applying a mode briefly interrupts
output. A 60 Hz HDMI signal may carry a 30 fps camera feed: the UI shows those
rates separately. It does not increase the camera frame rate.

**Preview:** this is a small live JPEG preview for framing and monitoring, not a
broadcast stream. Preview does not change the HDMI mode. Disable preview to
reduce processing and network traffic.

**History:** choose bitrate, frame rate or temperature, and hover over the chart
to inspect a sample. Shading marks periods without live video. History is cleared
when the application restarts; it does not write continuously to the SD card.

**Wi-Fi:** enter the SSID and a new password, then apply. Reconnect your computer
using those credentials. HDMI reception continues during the network change.
The web interface is intended for your private local network and has no separate
login; do not expose port 8090 to the Internet.

## Update without reflashing

Use the release bundle for your Pi OS architecture:

| Pi OS | Bundle |
| --- | --- |
| 64-bit | `ungoggled-0.2.0-aarch64.tar.gz` |
| 32-bit legacy installation | `ungoggled-0.2.0-armv7l.tar.gz` |

Copy it to the Pi, unpack it, and run its installer:

```sh
scp ungoggled-0.2.0-aarch64.tar.gz root@192.168.50.1:/tmp/
ssh root@192.168.50.1
cd /tmp
tar -xzf ungoggled-0.2.0-aarch64.tar.gz
cd ungoggled-0.2.0-aarch64
./install.sh
```

The installer verifies checksums, preserves settings/images/Wi-Fi, switches to
the new application release and checks the HTTP API. If that check fails, it
restores the previous application release when one exists. Updating briefly
interrupts video and clears the in-memory history. A read-only legacy root
filesystem must be remounted writable before installing or changing settings.

## Manual installation on Raspberry Pi OS Lite

Install the runtime packages:

```sh
sudo apt update
sudo apt install gstreamer1.0-tools gstreamer1.0-plugins-base \
  gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-libav curl
```

Add this to `/boot/firmware/config.txt` and reboot:

```ini
[all]
dtoverlay=dwc2,dr_mode=peripheral
```

Then install a release bundle as above, using your own Pi login and
`sudo ./install.sh` when logged in as a non-root user. Use the Pi's existing network address to
open port 8090. The application installer preserves your network configuration;
the preconfigured SD image additionally supplies the ungoggled access point.
The receiver needs exclusive use of the USB device controller and HDMI display.

## Build from source

Requires Rust stable with edition 2024 support, Node.js and npm.

```sh
cd web
npm ci
npm run build
cd ..
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

On the Pi, install the resulting binary and web assets with:

```sh
sudo scripts/install-pi.sh target/release/ungoggled
```

To cross-compile release bundles on Linux:

```sh
rustup target add aarch64-unknown-linux-musl
scripts/package-release.sh aarch64-unknown-linux-musl
```

Build the SD image with `scripts/build-image.sh`. It uses the checksum-pinned
official base image and installs packages inside an isolated ARM64 filesystem.
The Linux build host needs QEMU's registered `qemu-aarch64` binfmt handler,
subordinate UID/GID mappings, `unshare`, `newuidmap`, `newgidmap`, e2fsprogs,
curl, xz, Rust and Node.js. It does not write to a physical SD card. Outputs
are written to `dist/`; intermediate files stay in `build/`.

For a hardware-free integration check, install GStreamer with its software H.264
and JPEG plugins, then run:

```sh
cargo build
python3 tests/smoke.py target/debug/ungoggled /path/to/captured-video.h264
```

## Service and files

The service and persistent paths retain their original `dji-hdmi` names for
compatibility with existing installations. The application executable is
`ungoggled`.

| Location | Purpose |
| --- | --- |
| `dji-hdmi.service` | Receiver, renderer and HTTP server |
| `/opt/dji-hdmi/current` | Active application release |
| `/opt/dji-hdmi/releases/` | Installed releases |
| `/var/lib/dji-hdmi/` | Settings and fallback images |
| `/run/dji-hdmi/` | USB/video socket and GStreamer cache |
| `/etc/default/dji-hdmi` | Decoder/runtime configuration |
| `/etc/NetworkManager/system-connections/dji-hdmi.nmconnection` | Image's Wi-Fi AP profile |

```sh
sudo systemctl restart dji-hdmi
journalctl -u dji-hdmi -f
```

Rust handles USB, DJI framing, display management, settings and the Axum HTTP API.
GStreamer handles decoding and the small JPEG preview. The web application uses
React, TypeScript, Rsbuild and Tailwind. USB workers are supervised independently
of the persistent renderer.

The old 32-bit Buster image needs a separate `gst-omx` compatibility plugin;
modern Pi OS uses V4L2. Protocol research and legacy setup details are retained in
[FINDINGS.MD](FINDINGS.MD). Dependency attribution is in
[THIRD_PARTY.md](THIRD_PARTY.md).

Independent project, not affiliated with DJI or Cosmostreamer. Browser broadcast
streaming, recording management and remote camera controls are outside the
current feature set.
