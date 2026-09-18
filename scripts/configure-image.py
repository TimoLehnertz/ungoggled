#!/usr/bin/env python3
"""Prepare customization files. Called in the image's UID-mapped namespace."""
import pathlib, shutil, sys, uuid, tomllib
root=pathlib.Path(sys.argv[1]).resolve()
project=pathlib.Path(__file__).resolve().parent.parent
# Defaults are documented in README.md and identical for every image build.
values = {
    'WIFI_PASSWORD': 'ungoggled',
    'SSH_PASSWORD': 'ungoggled',
    'AP_UUID': str(uuid.uuid4()),
    'RADIO_COUNTRY': 'DE',
}
# Remove obsolete per-build credential files when reusing an older build tree.
for obsolete in ('build/image/credentials.json', 'dist/dji-hdmi-credentials.txt'):
    (project / obsolete).unlink(missing_ok=True)
(root/'tmp').mkdir(exist_ok=True)
p=root/'tmp/dji-image.env';p.write_text(''.join(f'{k}={v}\n' for k,v in values.items()));p.chmod(0o600)
for source,dest in [('scripts/check-image.sh','tmp/dji-check-image.sh'),('scripts/provision-image.sh','tmp/dji-provision.sh'),('scripts/firstboot.sh','tmp/dji-firstboot.sh'),('deploy/dji-hdmi-firstboot.service','tmp/dji-firstboot.service')]:shutil.copyfile(project/source,root/dest)
version=tomllib.loads((project/'Cargo.toml').read_text())['package']['version']
bundle=project/f'build/releases/ungoggled-{version}-aarch64'
shutil.rmtree(root/'tmp/dji-release', ignore_errors=True)
shutil.copytree(bundle,root/'tmp/dji-release',dirs_exist_ok=True)
resolv=root/'etc/resolv.conf'
if resolv.is_symlink():resolv.unlink()
resolv.write_text(pathlib.Path('/etc/resolv.conf').read_text())
policy=root/'usr/sbin/policy-rc.d';policy.write_text('#!/bin/sh\nexit 101\n');policy.chmod(0o755)
