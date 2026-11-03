#!/bin/sh
set -eu
module="minifuse_mod/$2"
case "$1" in
    install)
        if [ ! -e "/var/lib/dkms/$module/source" ]; then
            dkms add "$module"
        fi
        if [ -d "/lib/modules/$(uname -r)/build" ]; then
            dkms install "$module" -k "$(uname -r)"
        else
            echo "Install headers for the running kernel, then run sudo dkms autoinstall." >&2
        fi
        ;;
    remove)
        if [ -e "/var/lib/dkms/$module/source" ]; then
            dkms remove "$module" --all
        fi
        ;;
    *) exit 2 ;;
esac
