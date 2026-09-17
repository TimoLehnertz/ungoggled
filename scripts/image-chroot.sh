#!/bin/bash
# Runs inside an unprivileged user/mount/PID namespace. binfmt QEMU handles ARM64.
set -euo pipefail
root=$(realpath "${1:?root filesystem directory}")
shift
[[ "$root" != / && -f "$root/etc/os-release" ]]
mount --make-rprivate /
mount --rbind /dev "$root/dev"
mount -t proc proc "$root/proc"
mount --rbind /sys "$root/sys"
mount -t tmpfs tmpfs "$root/run"
trap 'umount -R "$root/run" "$root/sys" "$root/proc" "$root/dev" 2>/dev/null || true' EXIT
exec_cmd=(chroot "$root" /usr/bin/env DEBIAN_FRONTEND=noninteractive PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin)
"${exec_cmd[@]}" "$@"
