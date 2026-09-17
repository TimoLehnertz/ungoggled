# Verified investigation: Goggles 3 → Pi 4 → HDMI

Updated: 2026-09-16. Original notes reviewed at `/home/timo/dev/FINDINGS.MD`.
This document supersedes those notes where conclusions differ.

## What Cosmostreamer is

Cosmostreamer is a commercial hardware/software video bridge. Its Goggles product
receives a compatible DJI goggles feed, outputs it over HDMI and optionally
re-streams video, adds telemetry, and controls supported camera functions.
The vendor explicitly lists Goggles 3, O3 and O4 Pro. Those are **vendor claims**,
not proof that our independent implementation works.

- Product: https://cosmostreamer.com/products/djigoggles2/
- Wiring and hardware: https://cosmostreamer.com/products/djigoggles2/diy/

## Audit of the previous findings

| Earlier claim | Result of verification |
| --- | --- |
| PC enumerates Goggles 3 as `2ca3:0020` | Confirmed through local sysfs; Pi host enumeration independently agrees. |
| Pi is a Raspberry Pi 4, runs 32-bit Cosmostreamer | Confirmed `armv7l`, kernel `5.10.63-v7l+`, Raspbian 10 Buster. |
| Pi USB-C uses `dwc2` peripheral mode and gadgetfs | Confirmed boot config, UDC, module, device nodes and kernel logs. |
| AOA alone starts video automatically | Unsupported and contradicted by public protocol implementations. AOA establishes transport; DJI registration/keepalives still matter. |
| No open implementation exists | Too broad. There is a working, explicitly restricted G2/O3 implementation and an N3/O4 mobile protocol reference. Neither proves G3 compatibility. |
| Simply present an accessory PID | Not sufficient on the G2 reference: it needed the host-driven AOA transition first. G3 later resumed an already-negotiated session with the accessory PID; a fresh cold boot remains a separate test. |
| FunctionFS is required/preferred | Both drivers are available. Live testing exposed lost AOA setup events in gadgetfs on this kernel; FunctionFS completed all six AOA strings, START, re-enumeration, and live O4 reception. No kernel replacement was needed. |
| Pi 5 cannot do this | Vendor does not support it and it lacks the Pi 4 H.264 hardware decoder. That is not proof that software decoding is impossible. Pi 5 is outside our target. |
| Pi has a display connected | Driver status alone is insufficient: this image forces a 640×480 output and initially returned **zero EDID bytes**. UI now distinguishes monitor detection from connector status. |
| Invalid Cosmostreamer license / SD-card corruption | Not assumed as a protocol explanation. Kernel did report unclean FAT filesystem on mmcblk0p1; no filesystem repair or license alteration performed. |
| Previous investigation restored all state | Not fully true: two stopped shell processes from the earlier traces remained. We did not resume these obsolete tracing jobs. |

The original detailed IF4/DUML routing, SDK registration theories, license state,
and RNDIS conclusions have **not** been reproduced in this session and must not
be treated as established facts.

## Independently checked protocol sources

1. Android AOA specification:
   https://source.android.com/docs/core/interaction/accessories/aoa
   Requests 51 (protocol), 52 (identifying strings), 53 (start), then USB
   re-enumeration. `18d1:2d00` is accessory-only; `2d01` includes an ADB interface.
   The first successful G3 bench run used **2d01** as a compatibility experiment, with a single accessory interface and no ADB function. The specification-correct **2d00** subsequently completed full AOA negotiation and live reception on September 16. It is the source default and the current deployed setting; cold-accessory flags are no longer used.
2. Linux gadgetfs kernel implementation and UAPI, checked against v5.10:
   https://github.com/torvalds/linux/blob/v5.10/drivers/usb/gadget/legacy/inode.c
   https://github.com/torvalds/linux/blob/v5.10/include/uapi/linux/usb/gadgetfs.h
   Control ACK for host-to-device setup is a zero-length **read**, not write.
   Unsupported requests stall by opposite-direction I/O. Events are 12 bytes on
   both 32- and 64-bit systems. FS bulk max-packet is 64; HS is 512.
3. DJI mobile protocol at commit `50c71b65fdb6825783f724e5cd3c1f09c2aa88ce`:
   https://github.com/samuelsadok/dji_protocol/blob/50c71b65fdb6825783f724e5cd3c1f09c2aa88ce/usb_mobile_protocol.md
   https://github.com/samuelsadok/dji_protocol/blob/50c71b65fdb6825783f724e5cd3c1f09c2aa88ce/scripts/video_out_mobile.py
   This is an **iOS/MFI** reference tested on N3 and O4 Pro, not an AOA G3 test.
4. G2/O3 Pi endpoint at commit `64c5801ef0201a7a6070332df42e77d208fe10b2`:
   https://github.com/dr00min/o3-usb-decoder/tree/64c5801ef0201a7a6070332df42e77d208fe10b2
   It expressly excludes G3 and O4 support. Useful independent evidence for
   AOA framing, identity responses and host-driven accessory negotiation.
5. N3 Android implementation:
   https://github.com/jtm2kfpv-git/DJI-Unchained-VOC
   Its source verification script checks byte equality to the reference packets;
   that does not independently establish that their declared lengths are right.

## Important correction in the public N3 reference

Both documented `MAGIC_PACKETS` contain two extra bytes after the DUML frame:

- Packet 1 declares 45 DUML bytes, supplies 47. The valid 45-byte prefix already
  ends with CRC `d0 93`. Extra suffix `92 3a` is outside the declared frame.
- Packet 2 declares 27 DUML bytes, supplies 29. The valid 27-byte prefix already
  ends with CRC `58 a6`. Extra suffix `34 18` is outside the declared frame.

Confirmed using independent Rust bitwise CRCs and the G2 project's Python CRC
implementation. Copying the alleged payloads literally includes an old checksum
as application data. Our implementation uses the correctly bounded payloads and
recomputes both checksums. Tests compare complete output to both valid prefixes.
Whether extra trailing bytes are tolerated by the device does not make them part
of the declared DUML message.

Our initial minimal session sends `00:99` to address `0x28` and `00:88` to `0x3c`,
every three seconds, with fresh sequence numbers. The G2 AOA reference sends control on `0x7530`; the N3 Android application
and iOS reference send on `0x5749`. The prototype now defaults to `0x5749`
(decimal 22345), configurable with `--control-port`. Incoming telemetry can
arrive on `0x7530`. Video uses `0x574a`.
**Verified on the attached G3 + O4 Pro:** these corrected registrations produce live video over AOA. Replies include `28→02 00:99` and `3c→02 00:88`. We do not replay the G2 project's entire captured app session,
which includes unrelated commands.

## Hardware evidence so far

| Test | Result |
| --- | --- |
| ARM musl executable on supplied Buster Pi | PASS: runs, reports controller and DRM diagnostics. |
| gadgetfs descriptor binding | PASS: independent Rust worker binds `fe980000.usb`, endpoint nodes appear. |
| React static UI and status API over Wi-Fi | PASS: HTTP 200 at port 8090. |
| Generated H.264 1920×1080, 60 fps → `v4l2h264dec` → fakesink | PASS: parsed/decoded caps match, reaches EOS without errors. |
| Same generated clip → `kmssink sync=false` | PASS at software pipeline level: decoded caps, PLAYING, EOS. No visual confirmation yet. |
| `kmssink force-modesetting=true` on this legacy image | FAIL: cannot configure video mode. Removed forced modesetting; overlay path succeeds. |
| Goggles attached to Pi USB-A | Confirmed `2ca3:0020` on xHCI port `1-1.2`; wrong USB role. Requested move to Pi USB-C. |
| Goggles 3 + O4 Pro live USB | PASS: FunctionFS AOA handshake and approximately 6–7 Mbps H.264, 1920×1080 High level 5.2, with zero framing resync bytes over more than 15 minutes. |
| O4 Pro live hardware decode → HDMI | PASS: owner confirmed continuous live camera feed. After stopping competing media helpers and enabling a writable plugin cache, approximately 30 fps is measured. |
| Goggles 3 + O3 live USB and HDMI | PENDING hardware test. |
| Receiver process replacement after successful negotiation | PASS: normal AOA negotiation with default `18d1:2d00` returns to live streaming without a goggles reboot. Earlier cold-accessory experiments are no longer necessary. |
| Monitor attached after headless start | PASS: Rust DRM mode selection upgraded 1024×768 fallback to 1920×1080 at 60 Hz while retaining 1080p30 camera input. |
| Low-power-state toggles and clean feed | PASS: owner retested full-HD clean output and toggled Auto Low-Power State off/on, reporting “All works well.” Exact blackout duration was not measured. |
| Pi reboot with Goggles 3 + O4 Pro connected | PASS: standalone Wi-Fi and receiver start automatically; full AOA negotiation, 1080p input, one decoder start and zero USB resync bytes observed. |
| Physical USB reconnect / air-unit power cycle / O3 switch | Separate physical tests still pending. |

The Pi clock is set to February 2020. Its journal timestamps are not the test
date. Capture bounded samples and use session-relative statistics.

## Implementation decisions

- Rust owns USB via configfs/FunctionFS, framing, control session, worker supervision and HTTP API. The legacy gadgetfs experiment remains selectable for diagnosis.
- GStreamer runtime handles the Pi hardware decoder and DRM overlay. This avoids
  linking a modern Rust GStreamer dependency against the old Pi runtime.
- Queue is bounded. Decode restarts at an H.264 SPS after loss; arbitrary compressed
  chunks are not dropped and then presented as an intact stream.
- Raw capture is opt-in and capped, so it cannot fill the Pi RAM filesystem
  without a deliberate larger limit.
- No Pi OS replacement was needed. A temporary development service runs from
  `/tmp`; a dedicated-install script is supplied separately.


## Wiring clarification from the owner

The previous working Cosmostreamer USB-A setup was **Goggles V2 + O3**,
not Goggles 3 + O4 Pro. Thus it does not demonstrate a G3 USB-A video path.
The owner prefers charging while connected but is willing to assess USB-C
once its benefits are established. G3 has now enumerated our Pi peripheral
at USB high speed and sent AOA GET_PROTOCOL and SEND_STRING requests.
The first run exposed gadgetfs EIDRM on closely spaced control requests;
recovering from a superseded setup request is now implemented. Full AOA
transition and live O4 video subsequently succeeded with FunctionFS.

### FunctionFS transition

After a goggles reboot, all observed gadgetfs failures occur during the fast
SEND_STRING sequence; some requests are overwritten and the final START is not
processed. Catching EIDRM alone did not fix the stalled control state. The new
FunctionFS transport delegates vendor requests with ALL_CTRL_RECIP and
CONFIG0_SETUP, reads each OUT data phase explicitly, and changes PID only after
ACKing START. It is bound successfully on the existing Pi kernel.

## O4 decode investigation and legacy runtime workaround

The first USB capture is an ordinary 1920×1080, 8-bit 4:2:0 H.264 High stream.
The SPS advertises level 5.2 and 30 fps timing; timing metadata alone does not
prove the measured frame rate. It decodes correctly in software on the PC.
Captures start midstream, so errors before the first SPS/PPS are expected.

On this **Buster / GStreamer 1.14.4** image:

- `v4l2h264dec` fails on the O4 capture with no valid frames and early EPIPE/EOS.
  Rewriting level 5.2 to 4.2 did **not** fix it. We do not modify the SPS in the application.
- FFmpeg's `h264_v4l2m2m` and `h264_mmal` both decode the same captured footage,
  establishing that the Pi's H.264 hardware can decode it.
- The installed `omxh264dec` fails while creating `OMX.broadcom.egl_render`.
- A separate build of upstream **gst-omx 1.14.4 without GStreamer GL support**
  removes that unnecessary EGL component. It decodes the original, unmodified
  O4 stream and renders through `kmssink`. No vendor executable is used.
- The isolated plugin lives at `/tmp/dji-hdmi/plugins/libgstomx.so`; the development
  unit sets `GST_PLUGIN_PATH` and uses `--decoder omxh264dec`.
- A brief monitor test reported by the owner showed captured camera footage.
  This was file playback, not yet a visual confirmation of the continuous live path.

This is an old-image compatibility workaround. Use a current Pi OS with its
V4L2 decoder for a fresh installation, and test that runtime rather than assuming
this Buster result applies to all GStreamer versions.

Source archive (unmodified):
https://gstreamer.freedesktop.org/src/gst-omx/gst-omx-1.14.4.tar.xz
SHA256: `969870e75c1f75c96f8783530e2c2932fc3afbfd976eb0c466f51dae268ea3d4`

Related upstream evidence about old V4L2 plugin compatibility:
https://github.com/FDH2/UxPlay/wiki/Gstreamer-Video4Linux2-plugin-patches
https://github.com/raspberrypi/linux/issues/4592

The control UI now uses `fpsdisplaysink` rendered-frame counters as evidence of
output, rather than GStreamer's PLAYING state alone. The video queue has a 2 MiB
byte limit as well as a message limit; overflow discards queued compressed data
and resumes at a fresh SPS. USB reception never waits for the video decoder.

### Monitor connection and subsequent stability work

The owner confirmed no monitor was attached for the earlier zero-EDID readings.
On connection the Pi returned 256 EDID bytes and listed 1920×1080 modes.
The existing boot mode remained 640×480; decoded input resolution must not be
confused with the current HDMI signal resolution.

The live OMX path ran approximately 24–25 fps for more than two minutes before
monitor testing. It later stalled and repeatedly overran its queue. Further
inspection found active Cosmostreamer display, second-camera, screen-capture,
audio and recorder helpers. These were stopped, leaving networking and the
hardware watchdog running. Their process supervisors were paused as well.

GStreamer was also rescanning plugins on every restart because its usual cache
location is on the read-only filesystem. The test unit now sets
`GST_REGISTRY=/tmp/dji-hdmi/gst-registry.bin`; the supplied systemd unit uses its
own writable runtime directory. After these changes, the live output reported
approximately **29 fps**, with no USB resync bytes. This is an observation from
the latest short run, not a completed sustained or hotplug validation.

A transient FunctionFS bulk-write EAGAIN had restarted one USB session. The
source now retries temporary backpressure, with a two-second deadline, and has
a regression test covering interrupted, partial and temporarily blocked writes.
This USB retry change is deployed on the Pi.

During the monitor test the Pi became unreachable from the PC. Reconnecting the
PC Wi-Fi association restored access; Pi uptime and RAM files survived, so this
was not a Pi reboot. The owner had only reconnected the goggles USB cable. The
underlying cause of the Wi-Fi interruption is unconfirmed. A permanent installation was subsequently staged on September 16 (see below);
the current development service still runs from `/tmp` until reboot.

## September 16: full-HD output and transition recovery

The owner confirmed live HDMI but reported low resolution, long blackouts when
toggling Auto Low-Power State, and embedded goggles OSD. Two bounded O4 captures
both had the same 1920×1080 SPS with 30 fps timing. This does not prove that every
transmission-mode transition preserves those parameters.

The receiver now owns the HDMI primary framebuffer through Rust DRM ioctls and
selects progressive 1920×1080 at up to 60 Hz when advertised by the monitor.
GStreamer renders the camera onto an overlay. A background EDID probe upgrades
a headless fallback when the monitor is attached; probing on the decoder thread
was found to block long enough to disrupt playback on this old kernel. HDMI
status now reports output width, height and refresh separately from input caps.
The live hotplug test upgraded to 1080p60 and resumed approximately 30 fps output.

The H.264 path parses NAL boundaries across USB reads, detects changed SPS, and
restarts after an actual input gap or missing decoded frames. Input gaps use
arrival timestamps, so slow decoder processing cannot manufacture a fake gap.
Repeated SPS does not trigger a restart. Blackout duration during actual goggles
mode changes remains subject to the physical test.

OSD is embedded in the raw USB capture, including goggles menus, not added by
our HDMI renderer. Cosmostreamer documents Camera View Recording OFF under
Settings → Camera → Advanced Camera Settings for a clean feed:
https://cosmostreamer.com/products/djigoggles2/diy/#Clean_HDMI_video_without_OSD.3F
The owner has now set this, and `research/captures/o4-pro-clean.h264` contains
1920×1080 video without the previously visible OSD. The extracted first frame
was visually checked on the PC. No undocumented camera-setting command
or pixel masking is implemented.

### Persistent installation staged

The receiver, React assets and legacy OMX plugin are installed under `/usr/local`.
The corresponding unmodified gst-omx source archive and build script are saved
under `/usr/local/share/dji-hdmi/source`. The systemd receiver uses a writable
registry under `/run/dji-hdmi` and settings in `/etc/default/dji-hdmi`.

For this specific Buster image, `scripts/install-bench-pi.sh` saves the existing
hostapd configuration and installs a standalone AP unit using the existing
`wifi0` udev name and 192.168.50.1 address. Existing dnsmasq stays enabled.
Cosmostreamer startup in rc.local is replaced with an empty script; the old
rc.local and dhcpcd.conf are backed up in `/var/lib/dji-hdmi/previous-boot`.
PID 1 is configured to feed the hardware watchdog after reboot. The live
watchdog owner and current networking processes were left running.

Units are enabled and syntax-checked, but no reboot has yet validated this boot
configuration. Root was returned to read-only after installation. O3 remains
untested. Do not describe these pending hardware checks as completed.

SSH connections later stalled despite a working HTTP API. A new SSH connection
with `IPQoS=none` completed immediately and sustained an 8 MB capture transfer.
This workaround is verified; the precise Wi-Fi/DSCP handling fault is not.

The live receiver was subsequently switched from `dji-hdmi-dev.service` to the
installed `dji-hdmi.service`. Its preparation step and process start succeeded.
The new build also logs the reason for camera-gap, SPS-change and monitor-mode
restarts, and exposes `decoder_starts` through the status API.

The installed service then completed normal AOA negotiation and sustained
1080p HDMI output at approximately 29.97 fps for over two minutes with
`decoder_starts: 1` and zero USB resynchronization bytes. Local-network HTTP
access was intermittent during this period even while SSH and loopback API
requests worked; it subsequently recovered. Root cause remains unconfirmed.

### Owner confirmation, September 16

The owner completed the requested HDMI test, including clean video, image
sharpness, and Auto Low-Power State off/on transitions, and reported that all
works well. The monitor was disconnected afterwards. This confirms the O4 Pro
visual test; it does not establish a numerical blackout duration or O3 support.

### Reboot validation, September 16

After `systemctl reboot`, the Pi returned with its standalone AP and installed
receiver active. The PC rejoined the saved Wi-Fi network. The receiver performed
all six AOA identifying-string exchanges and resumed 1920×1080 O4 video without
a manual application restart. Status showed one decoder start, zero discarded
USB framing bytes, and approximately 30 output fps over the observed run.
No monitor was connected: HDMI used a 1024×768 fallback; the prior hotplug test
already verified upgrading to 1080p. This reboot is an API/decoder verification,
not an additional visual HDMI test.

The boot journal confirms PID 1 owns the Broadcom hardware watchdog with a
10-second timeout. A separate Cosmostreamer `splashscreen.service` was discovered
starting its omxiv framebuffer viewer outside rc.local; it was stopped and
disabled, and the migration script now handles that service. Video continued.

The pre-existing rpi-eeprom-update service fails its package checksum check on
this image. This is separate from the receiver; no EEPROM flashing or repair
was attempted. Receiver, hostapd and DHCP startup succeeded. O3 compatibility
remains pending a hardware test.

### Owner confirmation: O3 and O4

After the earlier O3-pending notes above, the owner tested both O3 Air Unit and
O4 Air Unit Pro and reported both working flawlessly. This establishes support
on the original Pi/Buster installation; it is not a physical Trixie validation.

### Version 0.2 validation, September 17

The Rust renderer now belongs to the long-lived server. The USB worker sends
sequence-numbered compressed chunks over a bounded Unix datagram path. A
persistent primary DRM framebuffer displays the selected fallback while
GStreamer uses an overlay for camera video. Uploaded PNG/JPEG/WebP images are
converted to bounded-size PNGs. Settings/images persist on disk; the 1,800-sample
history and JPEG preview remain in RAM.

O4 Pro hardware observations on the supplied Buster Pi:

- Input: 1920×1080, nominal 30 fps; measured approximately 29–30 fps.
- Output: approximately 30 rendered fps; HDMI upgraded from headless 1024×768
  to 1920×1080 at 60 Hz when the monitor was connected.
- Browser JPEG preview: 640×360, HTTP 200, clean image inspected locally.
- Owner visually confirmed live HDMI, “NO SIGNAL” after unplugging goggles,
  and live recovery after reconnecting. The monitor was then disconnected.
- Stream history recorded the loss and recovery. Temperature was around 45 °C.
- An initial regression came from kmssink/libdrm searching unrelated DRM drivers
  longer than the new input timeout. Setting `driver-name=vc4` eliminated that
  probe delay; observed steady playback then used one decoder until hotplug or
  USB reconnection.
- The release installer replaced the persistent systemd application successfully.
  Legacy root is now writable so settings and Wi-Fi changes survive reboot.

Automated checks: 15 Rust tests, Clippy with warnings denied, TypeScript/Rsbuild,
and an HTTP integration test covering upload/conversion/selection, controls,
settings, captured H.264 decoding, preview and expiry after signal loss.

The custom SD image uses the official September 15, 2026 Raspberry Pi OS Lite
ARM64 Trixie image, SHA-256
`cdf4f3bfac35ae947b46e4e767f935453810549779ac3290e05a6754aee627e5`.
Its build validates systemd services, user/sudo permissions, NetworkManager's
AP keyfile, required GStreamer plugins and the ARM64 HTTP server under QEMU.
Filesystem extraction through debugfs drops special mode bits; the build now
restores those bits from the original inode metadata before packaging.

A physical boot of the new image and V4L2 hardware decoding on its 6.18 kernel
remain untested. The new v0.2 renderer has been tested with O4; the owner's O3
confirmation predates this renderer update. Manual HDMI mode selection and the
new image's NetworkManager AP still need physical checks on that image.

The legacy hostapd Wi-Fi API was also exercised with the current SSID and
password. It applied successfully, returned no error, and the AP remained
reachable. The decoder-start count stayed unchanged across the network restart.

### Image login defaults, September 18

At the owner's request, newly built images now use the documented defaults:
Wi-Fi SSID `DJI-HDMI`, Wi-Fi password `djistreamer`, and SSH `root` / `root`.
The separate credential file and generated `dji` account have been removed.
Image validation checks the NetworkManager password, the root password hash,
and sshd's effective root/password-login policy. SSH host keys are still
created separately on each device's first boot. Application-only updates
continue to preserve the installed device's existing credentials.
