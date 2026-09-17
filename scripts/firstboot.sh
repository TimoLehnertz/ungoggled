#!/bin/bash
set -euo pipefail
state=/var/lib/dji-hdmi
mkdir -p "$state"
# Each flashed card receives its own SSH identity.
ssh-keygen -A
rfkill unblock wifi || true
if [[ -r /etc/dji-hdmi/radio-country ]]; then
    iw reg set "$(cat /etc/dji-hdmi/radio-country)" || true
fi
if [[ ! -e "$state/.initialized" ]]; then
    rootdev=$(findmnt -n -o SOURCE /)
    rootdev=$(readlink -f "$rootdev")
    name=${rootdev##*/}
    if [[ -b "$rootdev" && -r /sys/class/block/$name/partition ]]; then
        disk=$(lsblk -ndo PKNAME "$rootdev")
        part=$(cat "/sys/class/block/$name/partition")
        if [[ -n "$disk" ]]; then
            growpart "/dev/$disk" "$part" || true
            resize2fs "$rootdev"
        fi
    fi
    touch "$state/.initialized"
fi
