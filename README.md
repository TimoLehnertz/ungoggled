# DJI HDMI

An independent Rust receiver for DJI Goggles 3 USB video, with HDMI output on
Raspberry Pi 4 and a React + TypeScript + Rsbuild + Tailwind control interface.

**O4 Pro live reception and hardware decoding are running on the supplied Pi.**
The owner has confirmed continuous live HDMI. Full-HD output mode is now
verified through DRM, and a captured frame confirms clean video without OSD.
Visual transmission-mode-change testing is in progress.
The O3 hardware test is still pending. The supplied Buster image needs the decoder workaround described below. See [FINDINGS.MD](FINDINGS.MD)
for evidence, protocol corrections, and the hardware test matrix.

## What is implemented

- Rust USB peripheral using Linux FunctionFS: Android Open Accessory negotiation,
  re-enumeration, bulk endpoints, DJI framing, DUML CRCs, registration and replies.
- Bounded video queue and incremental H.264 framing; restart decoding on changed
  stream parameters, transmission gaps, queue overruns, or missing output frames.
- Select 1080p HDMI when supported by the display, independently of camera
  resolution. Upgrade automatically when a monitor is attached after startup.
- Hardware H.264 decoding through GStreamer `v4l2h264dec`, HDMI through `kmssink`.
  GStreamer is a separate supervised process, not linked into the Rust binary.
- An experimental `--transport gadgetfs` alternative is kept for investigation;
  closely spaced AOA setup requests exposed races on the supplied legacy kernel.
- Automatic receiver restart, browser start/stop/reconnect, status, diagnostics.
- Optional size-limited raw H.264 capture for protocol validation.

The UI controls the HDMI receiver. Browser video streaming, recording management,
software OSD removal, camera controls, Wi-Fi provisioning, and dual-HDMI mirroring are outside this
initial subset. No proprietary Cosmostreamer executables or license checks are
part of this implementation.

## Wiring

1. Power the Pi 4 separately through an appropriate GPIO/PoE power arrangement.
2. Connect **Goggles 3 USB-C → Pi USB-C** using a data cable. The rectangular Pi
   USB-A ports select the wrong USB role for this implementation.
3. Connect a display to **HDMI0**, the micro-HDMI port closest to USB-C.
4. Power and pair the O3 or O4 Pro; confirm a live picture inside the goggles.

On Raspberry Pi OS, add `dtoverlay=dwc2,dr_mode=peripheral` to the active boot
configuration (`/boot/firmware/config.txt` on recent images) and reboot. The
application needs exclusive use of the USB device controller and an available
DRM/KMS display. Stop competing receivers or graphical sessions first.

## Build

Requires Rust stable (edition 2024), Node.js and npm.

```sh
cd web
npm ci
npm run build
cd ..
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

For the supplied **32-bit** Pi image, cross compilation needs only the Rust target:

```sh
rustup target add armv7-unknown-linux-musleabihf
cargo build --release --target armv7-unknown-linux-musleabihf
```

The included linker configuration uses Rust's bundled linker. The binary has no
dynamic glibc dependency, so it also runs on the supplied Raspbian Buster image.
For a 64-bit Pi OS, build natively or configure the corresponding ARM64 target.

## Run on Pi

Install GStreamer runtime tools and plugins containing `h264parse`, `v4l2h264dec`,
`kmssink`, and `fpsdisplaysink` (on Raspberry Pi OS: `gstreamer1.0-tools`, `gstreamer1.0-plugins-good`,
`gstreamer1.0-plugins-bad`; software decoding additionally needs
`gstreamer1.0-libav`). Then:

```sh
sudo sh scripts/prepare-pi.sh
sudo ./dji-hdmi doctor
sudo ./dji-hdmi serve --listen 0.0.0.0:8090 --web-dir web/dist
```

Open `http://<pi-address>:8090`. For boot installation, run
`sudo sh scripts/install-pi.sh /path/to/the/pi-binary` from this project after
building the web assets. Installation expects a writable root filesystem.

Useful options:

```sh
# Receive and capture video without touching HDMI (cap is per USB session).
sudo ./dji-hdmi serve --output none --capture /tmp/live.h264 --capture-limit 16000000
# Decode without display for diagnosis.
sudo ./dji-hdmi serve --output test
# Override the decoder or DRM connector.
sudo ./dji-hdmi serve --decoder avdec_h264 --connector 32
```

`--output test` uses a real decoder and `fakesink`; it never simulates incoming
video or reports test imagery as a goggles feed.

## Development

Run the Rust server with `--no-autostart` on a PC without a gadget controller.
Run `npm run dev` in `web`; Rsbuild proxies `/api` to `127.0.0.1:8080`.

HTTP API:

| Request | Result |
| --- | --- |
| `GET /api/status` | Connection phase, byte counts, bitrate, decoder state |
| `GET /api/diagnostics` | USB controllers and HDMI detection/EDID/modes |
| `POST /api/start` | Enable receiver |
| `POST /api/stop` | Stop receiver and decoder |
| `POST /api/restart` | Reconnect USB session |

POST requests require `X-DJI-Control: 1`. This is a local-network interface with
no authentication; keep its listening address within your intended network.
No CORS permissions are enabled. The custom header prevents browser form-based
cross-origin control requests; it is not an authentication mechanism.

## Supplied Pi deployment

- Pi: `192.168.50.1`, web UI on port **8090**.
- Running service: `dji-hdmi.service`, installed under `/usr/local`.
- Enabled for next boot; the first reboot test is pending.
- Earlier `/tmp/dji-hdmi/` files remain available as a development fallback.
- Cosmostreamer camera supervisors and main process were paused for exclusive
  USB access. Its hardware-watchdog process remains running.
- Logs: `journalctl -u dji-hdmi -f`.

The generic installer is for a dedicated Pi OS installation. The separate
`scripts/install-bench-pi.sh` migrates this supplied Buster image: it preserves
the existing Wi-Fi credentials in `/etc/dji-hdmi/hostapd.conf`, starts hostapd
through `dji-hdmi-ap.service`, and replaces Cosmostreamer boot startup with our
receiver. Existing dnsmasq provides DHCP. PID 1 takes over the hardware watchdog
on the next boot. Old boot configuration is in `/var/lib/dji-hdmi/previous-boot`.
The receiver now runs from the permanent installation. Existing network and
watchdog processes remain active until reboot; boot recovery still needs to be
verified.

## Supplied legacy Buster image

The installed GStreamer 1.14 V4L2 decoder fails on the real O4 stream even though
it decodes the generated test clip. A separate gst-omx build without GL support
works with the Pi's legacy hardware decoder. `scripts/build-legacy-omx.sh`
reproduces this build when the development dependencies are installed. Set
`GST_PLUGIN_PATH` to its plugin directory and run with `--decoder omxh264dec`.
This workaround is specific to 32-bit legacy Pi OS with `/opt/vc` userland.

The current development service uses the standard AOA negotiation and accessory
PID `11520` (`18d1:2d00`), verified with live O4 Pro video. No compatibility flags
are required. Set `GST_REGISTRY` to a writable path on read-only systems to avoid
rescanning plugins on every decoder restart. A boot installation is now staged on the existing
Cosmostreamer OS; its reboot test is pending. The service reads decoder and plugin
settings from `/etc/default/dji-hdmi`.

## Clean video for livestreaming

In the goggles, select **Settings → Camera → Advanced Camera Settings → Camera
View Recording → OFF**. The [vendor documents this setting](https://cosmostreamer.com/products/djigoggles2/diy/#Clean_HDMI_video_without_OSD.3F)
for clean HDMI. Enable it to include the goggles display. This is controlled at
the source: the receiver forwards the encoded image and cannot remove graphics
already embedded in it. A captured 1920×1080 frame from this O4 setup is now free of OSD. Visual testing
while opening menus and changing transmission settings is still pending.

The UI reports camera resolution and HDMI signal separately. A 1080p60 HDMI
signal can carry a 30 fps camera feed; changing the output mode does not create
additional camera frames.
