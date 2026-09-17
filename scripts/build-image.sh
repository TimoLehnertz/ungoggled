#!/bin/bash
# Rootless Linux image build. Requires subordinate UID/GID mappings, binfmt
# qemu-aarch64, e2fsprogs, util-linux, curl, xz, Rust and Node/npm.
set -euo pipefail
cd "$(dirname "$0")/.."
source_url=https://downloads.raspberrypi.com/raspios_lite_arm64/images/raspios_lite_arm64-2026-09-15/2026-09-15-raspios-trixie-arm64-lite.img.xz
source_sha=cdf4f3bfac35ae947b46e4e767f935453810549779ac3290e05a6754aee627e5
mkdir -p build/cache build/image dist
base=build/cache/raspios-lite-arm64.img
if [[ ! -f "$base.xz" ]]; then curl -fL --retry 2 "$source_url" -o "$base.xz";fi
printf '%s  %s\n' "$source_sha" "$base.xz" | sha256sum -c -
[[ -f "$base" ]] || xz -dk "$base.xz"
bash scripts/package-release.sh aarch64-unknown-linux-musl
# Pinned image layout, verified against its MBR before reading partitions.
python3 - <<'PY'
import pathlib,struct
b=pathlib.Path('build/cache/raspios-lite-arm64.img').open('rb').read(512)
assert b[510:]==b'\x55\xaa'
assert struct.unpack_from('<II',b,446+8)==(16384,1048576)
assert struct.unpack_from('<II',b,462+8)==(1064960,4915200)
PY
[[ -f build/image/root.ext4 ]] || dd if="$base" of=build/image/root.ext4 bs=512 skip=1064960 count=4915200 status=none
if [[ ! -f build/image/rootfs/etc/os-release ]]; then
    mkdir -p build/image/rootfs
    unshare --user --map-root-user --map-auto --mount debugfs -R 'rdump / build/image/rootfs' build/image/root.ext4
fi
unshare --user --map-root-user --map-auto --mount python3 scripts/restore-image-modes.py build/image/root.ext4 build/image/rootfs
unshare --user --map-root-user --map-auto --mount python3 scripts/configure-image.py build/image/rootfs
unshare --user --map-root-user --map-auto --mount --pid --fork --kill-child scripts/image-chroot.sh build/image/rootfs /bin/bash /tmp/dji-provision.sh
unshare --user --map-root-user --map-auto --mount --pid --fork --kill-child scripts/image-chroot.sh build/image/rootfs /bin/bash /tmp/dji-check-image.sh
unshare --user --map-root-user --map-auto --mount chroot build/image/rootfs dpkg-query -W > dist/dji-hdmi-image-packages.txt
# Extract the original FAT boot partition using mtools from the ARM image.
dd if="$base" of=build/image/rootfs/tmp/dji-boot.fat bs=512 skip=16384 count=1048576 status=none
unshare --user --map-root-user --map-auto --mount chroot build/image/rootfs /bin/bash -c 'mkdir -p /boot/firmware; mcopy -s -i /tmp/dji-boot.fat "::*" /boot/firmware/'
unshare --user --map-root-user --map-auto --mount python3 - <<'PY'
from pathlib import Path
boot=Path('build/image/rootfs/boot/firmware')
p=boot/'config.txt';s=p.read_text();s+='\n[all]\n# DJI HDMI USB accessory controller\ndtoverlay=dwc2,dr_mode=peripheral\n';p.write_text(s)
p=boot/'cmdline.txt';parts=p.read_text().split();parts=[p for p in parts if not p.startswith(('init=','systemd.run=','systemd.run_success_action=','systemd.unit=','video=HDMI-A-1:'))];parts+=['video=HDMI-A-1:1920x1080@60D'];p.write_text(' '.join(parts)+'\n')
PY
unshare --user --map-root-user --map-auto --mount chroot build/image/rootfs /bin/bash -c 'mcopy -o -i /tmp/dji-boot.fat /boot/firmware/config.txt /boot/firmware/cmdline.txt ::/'
mv build/image/rootfs/tmp/dji-boot.fat build/image/boot.fat
# Remove build-only content and reset machine-specific state.
unshare --user --map-root-user --map-auto --mount python3 - <<'PY'
from pathlib import Path
import shutil
r=Path('build/image/rootfs')
for p in r.glob('qemu_*.core'):p.unlink()
for p in (r/'tmp').iterdir():
    if p.is_dir() and not p.is_symlink():shutil.rmtree(p)
    else:p.unlink()
# Boot files live on their FAT partition, not twice on the root filesystem.
for p in (r/'boot/firmware').iterdir():
    if p.is_dir():shutil.rmtree(p)
    else:p.unlink()
p=r/'etc/resolv.conf';p.unlink(missing_ok=True);p.symlink_to('/run/NetworkManager/resolv.conf')
PY
root_sectors=12582912 # 6 GiB, expanded to the card by firstboot.sh
root_image=build/image/final-root.ext4
truncate -s $((root_sectors*512)) "$root_image"
unshare --user --map-root-user --map-auto --mount mke2fs -q -t ext4 -F -L rootfs -d build/image/rootfs "$root_image"
e2fsck -fn "$root_image"
out=dist/dji-hdmi-0.2.0-pi4-arm64.img
truncate -s $(((1064960+root_sectors)*512)) "$out"
dd if="$base" of="$out" bs=512 count=16384 conv=notrunc status=none
dd if=build/image/boot.fat of="$out" bs=512 seek=16384 conv=notrunc status=none
dd if="$root_image" of="$out" bs=512 seek=1064960 conv=notrunc status=none
python3 - "$out" "$root_sectors" <<'PY'
import struct,sys
with open(sys.argv[1],'r+b') as f:f.seek(462+12);f.write(struct.pack('<I',int(sys.argv[2])))
PY
xz -T2 -3 -f "$out"
sha256sum "$out.xz" > "$out.xz.sha256"
printf 'Image: %s.xz\nDefault Wi-Fi and root login: see README.md\n' "$out"
