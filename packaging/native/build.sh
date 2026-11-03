#!/usr/bin/env bash
# Build a native desktop package on its target distribution.
set -euo pipefail
cd -- "$(dirname -- "$0")/../.."
if [[ $# != 3 ]]; then
    echo "Usage: $0 VERSION GUI_SOURCE OUTPUT_DIRECTORY" >&2
    exit 2
fi
version=$1
[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]
gui=$(realpath "$2")
output=$(realpath -m "$3")
[[ $(uname -m) == x86_64 ]]
. /etc/os-release
case "$ID:$VERSION_ID" in
    ubuntu:26.04|fedora:45) ;;
    *) echo "Unsupported package build target: $ID $VERSION_ID" >&2; exit 2 ;;
esac

work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT
stage=$work/root
mkdir -p "$output" "$stage"
cargo build --workspace --release --locked
make install DESTDIR="$stage"

driver="$stage/usr/src/minifuse_mod-$version"
install -d "$driver"
install -m644 kmod/*.h kmod/minifuse_mod.c kmod/Makefile kmod/dkms.conf kmod/LICENSE "$driver/"
sed -i "s/^PACKAGE_VERSION=.*/PACKAGE_VERSION=\"$version\"/" "$driver/dkms.conf"
install -Dm644 packaging/70-minifuse.rules "$stage/usr/lib/udev/rules.d/70-minifuse.rules"
install -Dm644 "$gui/packaging/minifuse-desktop-agent.service" "$stage/usr/lib/systemd/user/minifuse-desktop-agent.service"
install -Dm644 "$gui/packaging/io.github.srdiogo.MiniFuseControl.desktop" "$stage/usr/share/applications/io.github.srdiogo.MiniFuseControl.desktop"
install -Dm644 "$gui/src/minifuse_control/assets/minifuse-control.svg" "$stage/usr/share/icons/hicolor/scalable/apps/io.github.srdiogo.MiniFuseControl.svg"
install -Dm644 "$gui/LICENSE" "$stage/usr/share/doc/minifuse-linux/gui-LICENSE"
install -Dm644 THIRD_PARTY.md "$stage/usr/share/doc/minifuse-linux/THIRD_PARTY.md"
install -d "$stage/usr/lib/minifuse/minifuse_control"
cp -R "$gui/src/minifuse_control/"*.py "$gui/src/minifuse_control/qml" "$gui/src/minifuse_control/assets" "$stage/usr/lib/minifuse/minifuse_control/"
for entry in minifuse-control:app minifuse-desktop-agent:desktop; do
    cat > "$stage/usr/bin/${entry%:*}" <<EOF
#!/usr/bin/python3 -I
import sys
sys.path.insert(0, "/usr/lib/minifuse")
from minifuse_control.${entry#*:} import main
raise SystemExit(main())
EOF
    chmod 755 "$stage/usr/bin/${entry%:*}"
done
install -Dm755 packaging/native/dkms-lifecycle.sh "$stage/usr/lib/minifuse/dkms-lifecycle"

if [[ $ID == ubuntu ]]; then
    mkdir "$stage/DEBIAN"
    cat > "$stage/DEBIAN/control" <<EOF
Package: minifuse-linux
Version: $version-1
Architecture: amd64
Maintainer: MiniFuse for Linux contributors
Depends: libc6, libgcc-s1, dkms, udev, systemd, python3 (>= 3.12), python3-pyside6.qtquick, qml6-module-qtquick, qml6-module-qtquick-controls, qml6-module-qtquick-dialogs, qml6-module-qtquick-layouts, qml6-module-qtquick-templates, qml6-module-qtquick-window, qml6-module-qtqml-workerscript
Recommends: qt6-wayland, libnotify-bin
Conflicts: mf-service, minifuse-driver-dkms, mf-cli-platform
Description: MiniFuse controls, mixer and driver for Linux
 Includes the session service, Qt mixer and optional desktop activation agent.
EOF
    cat > "$stage/DEBIAN/postinst" <<EOF
#!/bin/sh
set -e
[ "\$1" != configure ] || /usr/lib/minifuse/dkms-lifecycle install '$version'
EOF
    cat > "$stage/DEBIAN/prerm" <<EOF
#!/bin/sh
set -e
case "\$1" in
    remove|upgrade) /usr/lib/minifuse/dkms-lifecycle remove '$version' ;;
esac
EOF
    chmod 755 "$stage/DEBIAN/"{postinst,prerm}
    dpkg-deb --root-owner-group --build "$stage" "$output/minifuse-linux_${version}-1_ubuntu26.04_amd64.deb"
else
    tar -C "$stage" -czf "$work/payload.tar.gz" usr
    rpmbuild -bb --define "_topdir $work" --define "minifuse_version $version" packaging/native/minifuse-linux.spec
    cp "$work/RPMS/x86_64/"*.rpm "$output/"
fi
