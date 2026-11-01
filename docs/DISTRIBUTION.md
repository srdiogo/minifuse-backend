# Distribution support and packaging roadmap

MiniFuse for Linux has one shared backend, a standalone application and an
optional Omarchy panel. The complete installer selects components; native
packages remain responsible for system integration, updates and removal.

## Available today

| Environment | Delivery | Evidence and limits |
|---|---|---|
| Arch Linux x86_64 | Complete installation kit and native package recipes | CI builds packages and driver; bounded MF4 hardware validation |
| Omarchy on Arch x86_64 | Same kit, with optional panel/floating setup | Reference: Omarchy 4.0.4-1, Quickshell 0.3.1, Qt 6.11.2 |
| Other distributions | Source for development | Complete installer and native packages not yet supported |

The kit's GUI packages target Python 3.14. Source GUI installation supports
Python 3.12–3.14 with its declared Qt dependency; this does not establish
driver/package support on another distribution. The application does not
require Omarchy. Current GUI/driver evidence is bounded to MiniFuse 4.

The operator confirmed immediate and post-reboot operation of the delivered
kit; the initial machine setup was not specified. See
[installation evidence](INSTALLATION_VALIDATION.md) and
[the reproduction checklist](REPRODUCING.md).

## Delivery sequence

### 1. Arch and Omarchy

- Publish the verified complete kit with exact source references and checksums.
- Keep `--profile gui`, `backend`, `omarchy` and `full` in the same installer.
- Prepare AUR submissions from reviewed tagged sources, with current `.SRCINFO`
  generated from each actual package recipe. There is no AUR publication yet.
- Validate package upgrades and removals, matching headers and existing-driver
  migration before declaring a broader supported release.
- Submit the panel to the Omarchy marketplace after validating the current
  marketplace host. The plugin remains a client with no privileged install hooks.

### 2. Ubuntu LTS and Debian

- Select and record exact distribution releases and kernels for the first matrix.
- Build separate `.deb` packages for the driver, session service and application.
- Integrate DKMS through Debian packaging helpers; keep the service unprivileged.
- Publish checksummed preview packages first, then an authenticated APT repository
  when the build, signing and update process is maintained.
- Add an installer adapter that uses those packages and their dependencies.

Debian's [`dh_dkms`](https://manpages.debian.org/testing/dh-dkms/dh_dkms.1.en.html)
handles module package integration and dependencies. The current Arch installer
must not invoke pacman or offer installation on these systems.

### 3. Fedora Workstation

- Select an explicit Fedora release/kernel matrix and build native RPMs.
- Validate the kernel-module packaging strategy and required compiler/header
  packages; Arch package assumptions do not carry over automatically.
- Use [COPR](https://developer.fedoraproject.org/deployment/copr/about.html)
  for reproducible RPM builds and an install/update repository.
- Add the installer adapter after install, update and removal acceptance.

Fedora Atomic desktops and other immutable systems require a separate backend
delivery design; Workstation packages alone do not establish their support.

### 4. Portable application distribution

Evaluate Flatpak for the GUI after native backend delivery is reliable. The
driver and session service stay installed on the host. Validate narrowly scoped
access to the owned service socket, file chooser/import/export behavior,
single-instance activation, theme and Wayland behavior in the sandbox. Follow
[Flatpak permissions guidance](https://docs.flatpak.org/en/latest/sandbox-permissions.html).
A portable GUI package is not a complete driver installation.

## Acceptance for each new target

Record the distribution, kernel/headers, architecture, package versions, source
commits and Qt/Python versions. Keep separate results for build-only CI, isolated
package tests and actual machine/hardware operation.

1. Install on a clean target and upgrade an existing supported installation.
2. Verify matching driver build, active-session access, service lifecycle and
   boot/login behavior. Cover module signing with Secure Boot where applicable.
3. Verify application launch and current-state reads without Omarchy; validate
   the panel separately when its tested host is present.
4. Exercise bounded hardware controls with readback and baseline restoration,
   without changing microphone power merely to test packaging.
5. Verify reboot, package update, kernel update and removal, preserving presets
   and unresolved operation evidence.
6. Check download integrity, package ownership, dependency constraints and
   actionable recovery when prerequisites or the device are missing.

Installation detection must stop before mutation on an unsupported target.
The installer must preserve its read-only mode, visible plan, fixed privileged
operations, backups and unprivileged user-session configuration. Firmware,
effects, automatic preset restore and changes to the host audio stack remain
outside the distribution work.
