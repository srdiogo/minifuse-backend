# Native packages for MiniFuse for Linux

Download the package for your distribution and `SHA256SUMS` from the
[release page](https://github.com/srdiogo/minifuse-backend/releases) or the
[package archive](https://github.com/srdiogo/minifuse-backend/tree/packages). These
packages target x86-64. Verify the downloaded files in the same directory:

```sh
sha256sum -c SHA256SUMS --ignore-missing
```

## Ubuntu 26.04 LTS

```sh
sudo apt update
sudo apt install "linux-headers-$(uname -r)"
sudo apt install ./minifuse-linux_*_ubuntu26.04_amd64.deb
```

## Fedora 45

```sh
sudo dnf install "kernel-devel-$(uname -r)"
sudo dnf install ./minifuse-linux-*.fc45.x86_64.rpm
```

If matching headers are unavailable, boot a kernel for which the distribution
provides headers before installing. Both packages use the distribution's Qt
and Python dependencies. DKMS builds for the running kernel when its headers
are present. The distribution's module-signing policy applies.

After installation, load the driver and start the service as your desktop user:

```sh
sudo modprobe minifuse_mod
systemctl --user daemon-reload
systemctl --user enable --now mf-service.service
minifuse-control
```

After upgrading an existing installation, run
`systemctl --user restart mf-service.service` to start the updated backend.

On a validated MF4, select **Application** in the desktop activation settings
and enable the optional `minifuse-desktop-agent.service` user unit for the A
button. The GUI and backend are included in the native packages.

## Arch Linux and Omarchy

Install the release's split Arch packages together with `sudo pacman -U`.
Use the backend, DKMS and GUI packages for the full mixer. See the
[Arch instructions](../packaging/arch/README.md) for kernel headers and
configuration. The optional Omarchy panel is packaged separately as a source
archive.

## Release workflow

Update `distribution/release.json` with a new version and the two companion
commits from their respective main branches. Label the pull request `release`
to build and check its packages. Merge the labeled PR into main to authorize
publication. The workflow rebuilds from the merge commit, checks the package
matrix and publishes a versioned prerelease with the companion revisions and
checksums after all jobs pass.

Native VM acceptance covers installation, DKMS build and load, the session API
and offscreen Qt startup. Physical device testing is separate. MF1, MF2 and 2 OTG features are enabled automatically after firmware, driver and readback checks.
