#!/bin/sh
# Only for the supplied Buster image, whose udev rule names onboard Wi-Fi wifi0.
set -eu
modprobe brcmfmac
udevadm settle --timeout=15
test -d /sys/class/net/wifi0
ip link set wifi0 up
ip address replace 192.168.50.1/24 dev wifi0
iw dev wifi0 set power_save off
