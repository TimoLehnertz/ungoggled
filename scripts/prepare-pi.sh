#!/bin/sh
# Configure the USB device side only. This script does not change the boot config.
set -eu
[ "$(id -u)" -eq 0 ] || { echo 'Run as root.' >&2; exit 1; }
modprobe dwc2
modprobe libcomposite
modprobe usb_f_fs
if ! ls /sys/class/udc/*/uevent >/dev/null 2>&1; then
    echo 'No USB device controller. Add dtoverlay=dwc2,dr_mode=peripheral to the Pi boot config and reboot.' >&2
    exit 1
fi
if ! mountpoint -q /sys/kernel/config; then
    mount -t configfs configfs /sys/kernel/config
fi
for element in h264parse kmssink fpsdisplaysink; do
    gst-inspect-1.0 "$element" >/dev/null
done
echo 'Configfs, FunctionFS and video plugins ready.'
