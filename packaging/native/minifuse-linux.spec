%global debug_package %{nil}
Name: minifuse-linux
Version: %{minifuse_version}
Release: 1%{?dist}
Summary: MiniFuse controls, mixer and driver for Linux
License: MIT AND GPL-2.0-only
URL: https://github.com/srdiogo/minifuse-backend
Source0: payload.tar.gz
Requires: dkms, systemd, systemd-udev, python3-pyside6, qt6-qtdeclarative
Recommends: qt6-qtwayland, libnotify
Conflicts: mf-service, minifuse-driver-dkms, mf-cli-platform
ExclusiveArch: x86_64

%description
Includes the session service, Qt mixer, DKMS driver and optional desktop agent.

%install
mkdir -p %{buildroot}
tar -xzf %{_topdir}/payload.tar.gz -C %{buildroot}

%posttrans
# Register the replacement after the previous RPM has removed its DKMS module.
/usr/lib/minifuse/dkms-lifecycle install '%{version}'

%preun
# A version upgrade removes only this package's DKMS version.
if [ "$1" -eq 0 ] || rpm -q --qf '%%{VERSION}\n' minifuse-linux | grep -Fxqv '%{version}'; then
    /usr/lib/minifuse/dkms-lifecycle remove '%{version}'
fi

%files
/usr/bin/mf-service
/usr/bin/minifuse-doctor
/usr/bin/minifuse-control
/usr/bin/minifuse-desktop-agent
/usr/lib/minifuse
/usr/lib/systemd/user/*.service
/usr/lib/udev/rules.d/70-minifuse.rules
/usr/src/minifuse_mod-%{version}
/usr/share/applications/io.github.srdiogo.MiniFuseControl.desktop
/usr/share/icons/hicolor/scalable/apps/io.github.srdiogo.MiniFuseControl.svg
/usr/share/licenses/mf-service
/usr/share/doc/minifuse-linux
