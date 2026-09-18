#!/bin/bash
# Bootstrap any older installation into the transactional updater.
set -euo pipefail
[[ $(id -u) == 0 ]] || { echo 'Run with sudo.' >&2; exit 1; }
cd "$(dirname "$(readlink -f "$0")")"
[[ $(uname -m) == aarch64 && $(getconf LONG_BIT) == 64 ]] || {
    echo 'This update requires 64-bit Raspberry Pi OS on a Pi 4.' >&2; exit 1;
}
exec ./bin/aarch64/ungoggled update-install --source "$PWD"
