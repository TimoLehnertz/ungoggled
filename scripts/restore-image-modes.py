#!/usr/bin/env python3
"""Restore special permission bits that debugfs rdump omits.

Run inside the same UID namespace as extraction. Read the base filesystem's
inode modes, never infer privilege bits from filenames.
"""
import concurrent.futures
import json
from pathlib import Path
import stat
import subprocess
import sys

image, root = Path(sys.argv[1]), Path(sys.argv[2])
cache = image.with_suffix('.special-modes.json')
if cache.exists():
    modes = json.loads(cache.read_text())
else:
    modes = {}
    pending = [('/', 2)]
    def listing(item):
        path, inode = item
        result = subprocess.run(['debugfs', '-R', f'ls -p <{inode}>', str(image)],
                                check=True, capture_output=True, text=True)
        return path, result.stdout
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
        while pending:
            children = []
            for parent, output in pool.map(listing, pending):
                for line in output.splitlines():
                    fields = line.split('/')
                    if len(fields) < 7 or fields[5] in ('.', '..', ''):
                        continue
                    inode, mode, name = int(fields[1]), int(fields[2], 8), fields[5]
                    path = parent.rstrip('/') + '/' + name
                    if mode & 0o7000:
                        modes[path] = stat.S_IMODE(mode)
                    if stat.S_ISDIR(mode):
                        children.append((path, inode))
            pending = children
    cache.write_text(json.dumps(modes))
for path, mode in modes.items():
    target = root / path.lstrip('/')
    if target.exists() and not target.is_symlink():
        target.chmod(mode)
print(f'Restored {len(modes)} special inode modes from base image')
