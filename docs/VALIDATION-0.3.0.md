# 0.3.0 validation — 18 September 2026

## Automated checks

- 26 Rust tests passed; Clippy with warnings denied and rustfmt passed.
- Two browser release/version selection tests passed; TypeScript compilation,
  production Rsbuild build and Prettier checks passed.
- HTTP/GStreamer smoke test passed: settings, image conversion/library,
  transmission history, real H.264 decoding, JPEG preview and signal-loss expiry.
- Update tests cover unsafe paths, links, duplicates, truncated/oversized archives,
  mismatched checksums, versions/architectures, multipart handling, migration,
  failed startup rollback, interrupted-transaction recovery and the commit boundary.
- Built universal archive verified with the Rust validator for ARM32 and ARM64.
- Browser checks covered release notice/notes/link, desktop/mobile layout, and
  the actual Pi upload/install/reconnect flow without JavaScript exceptions.

## Pi 4 running the flashed Trixie image

The owner's 0.2.0 image booted but had Wi-Fi disabled and no live video output.
SSH access over Ethernet established these causes:

1. `/var/lib/NetworkManager/NetworkManager.state` retained
   `WirelessEnabled=false` from the stock Lite image. `nmcli radio wifi on`
   restored the access point. The owner confirmed the network was visible.
   Provisioning now enables the saved radio state, sets rfkill's initial state,
   and writes the configured radio country into the boot command line.
2. The GStreamer 1.26.2 V4L2 decoder advertised H.264 levels only through 5.1.
   Goggles video advertised 5.2; a captured O4 stream and the live O3 stream
   failed caps negotiation before decoding. Setting the decoder negotiation
   metadata to 5.1 fixed this. Encoded bytes and SPS are unchanged.

The existing 0.2.0 installation was upgraded through the universal package's
manual bootstrap. A subsequent full browser upload and installation completed
in approximately 11.6 seconds from Install to the updater's success report.
The browser reconnected and reloaded automatically.

SHA-256 comparisons before/after confirmed identical application settings,
fallback images, NetworkManager profile, `/etc/shadow` and decoder configuration.
The Pi was then rebooted. Wi-Fi connected automatically, the recovery service
completed, HTTP was available on port 80, and the receiver resumed O3 without
manual USB intervention. No failed systemd units were reported.

Observed live O3 input: 1920×1080, approximately 30 fps, roughly 6–7 Mbps.
The renderer reported approximately 30 output frames/s at 1920×1080 / 60 Hz,
with a 640×360 JPEG browser preview. A captured O4 stream also decoded successfully
through the Trixie hardware decoder to EOS.

## Limits

- The owner could not reconnect an HDMI monitor after moving the Pi to Ethernet.
  This run confirms decoded/output frame counters and preview, not a visual
  HDMI check on Trixie. Earlier visual O3/O4 checks used the supplied Buster OS.
- The newly rebuilt 0.3.0 SD image was checked in an ARM64 chroot, including
  service configuration, permissions, Wi-Fi defaults, SSH login policy, plugins
  and HTTP startup. It has not itself been flashed in this run. The physical
  reboot test used the owner's flashed 0.2.0 image repaired and updated to 0.3.0.
- Failed-update and power-interruption recovery were tested with filesystem
  transactions and a fake service manager, not by cutting physical Pi power.
- The release workflow is added but has not been run or published to GitHub.

## ARM64-only release cleanup

After the initial validation above, release support was narrowed to Pi 4 with
64-bit Raspberry Pi OS. ARM32 is no longer built or accepted by the updater.
The final ARM64-only update archive passed the Rust validator, all 26 Rust tests
and Clippy. The earlier dual-architecture results above describe the prototype.
Old artifacts were moved out of `dist/` into `build/archived-dist/`; release
outputs now consist of the ARM64 SD image, ARM64 application update and their
checksum files. Intermediate image bundles and reports remain under `build/`.
