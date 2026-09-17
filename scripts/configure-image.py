#!/usr/bin/env python3
"""Prepare customization files. Called in the image's UID-mapped namespace."""
import json,os,pathlib,secrets,shutil,string,sys,uuid
root=pathlib.Path(sys.argv[1]).resolve()
project=pathlib.Path(__file__).resolve().parent.parent
stage=project/'build/image'
credentials=stage/'credentials.json'
if credentials.exists(): values=json.loads(credentials.read_text())
else:
    password=lambda:''.join(secrets.choice(string.ascii_letters+string.digits) for _ in range(18))
    values={'WIFI_PASSWORD':password(),'SSH_PASSWORD':password(),'AP_UUID':str(uuid.uuid4()),'RADIO_COUNTRY':'DE'}
    credentials.write_text(json.dumps(values));credentials.chmod(0o600)
(root/'tmp').mkdir(exist_ok=True)
p=root/'tmp/dji-image.env';p.write_text(''.join(f'{k}={v}\n' for k,v in values.items()));p.chmod(0o600)
for source,dest in [('scripts/check-image.sh','tmp/dji-check-image.sh'),('scripts/provision-image.sh','tmp/dji-provision.sh'),('scripts/firstboot.sh','tmp/dji-firstboot.sh'),('deploy/dji-hdmi-firstboot.service','tmp/dji-firstboot.service')]:shutil.copyfile(project/source,root/dest)
bundle=project/'build/releases/dji-hdmi-0.2.0-aarch64'
shutil.copytree(bundle,root/'tmp/dji-release',dirs_exist_ok=True)
resolv=root/'etc/resolv.conf'
if resolv.is_symlink():resolv.unlink()
resolv.write_text(pathlib.Path('/etc/resolv.conf').read_text())
policy=root/'usr/sbin/policy-rc.d';policy.write_text('#!/bin/sh\nexit 101\n');policy.chmod(0o755)
out=project/'dist/dji-hdmi-credentials.txt'
out.write_text(f"DJI HDMI image\n\nWi-Fi SSID: DJI-HDMI\nWi-Fi password: {values['WIFI_PASSWORD']}\nWeb interface: http://192.168.50.1:8090\nSSH: dji@192.168.50.1\nSSH/sudo password: {values['SSH_PASSWORD']}\nRadio country: {values['RADIO_COUNTRY']}\n\nThese credentials belong to this image build. Change Wi-Fi in the web UI\nand change the login password with passwd after flashing.\n")
out.chmod(0o600)
