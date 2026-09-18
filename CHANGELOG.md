# Changelog

## 0.3.0

- Serve the web interface on standard HTTP port 80.
- Upload and install software updates from the web interface, with upload progress and automatic reconnection.
- One complete update file supports Pi 4 installations running 64-bit Raspberry Pi OS, without installing intermediate application releases.
- Preserve compatible HDMI, preview and fallback settings. Keep uploaded images, Wi-Fi settings and login credentials.
- Restore the previous application and settings if the new version fails its startup check. Recover interrupted installations at boot.
- Check GitHub releases from the browser and show newer stable versions, release notes and download links.
- Fix V4L2 hardware decoder negotiation for Goggles streams advertising H.264 level 5.2, without changing the encoded video.
- Fix fresh-image Wi-Fi being disabled by the stock NetworkManager radio state, and persist the configured radio country at boot.
- Add semantic version management and automated release builds containing an SD-card image and an ARM64 update file.
- Halve the SD-card image: the root filesystem is sized to its contents plus working headroom, and documentation, translations and cached package lists the appliance never reads are left out. The partition still expands to the whole card on first boot.
- Ship only the current application release in the SD-card image, instead of also carrying the release directories of earlier builds.

## 0.2.0

- Live HDMI output for Goggles 3 with O3 Air Unit and O4 Air Unit Pro.
- Browser preview, HDMI controls, fallback images, Pi temperature and a 30-minute transmission history.
- Wi-Fi configuration and preconfigured Raspberry Pi OS Lite images.
- Rename the project to ungoggled. Default Wi-Fi SSID, Wi-Fi password and root password are `ungoggled`.
