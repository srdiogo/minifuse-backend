# MiniFuse Backend

**The shared Linux backend and control driver for MiniFuse Control and desktop integrations.**

[![Build and tests](https://github.com/srdiogo/minifuse-backend/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/srdiogo/minifuse-backend/actions/workflows/ci.yml)

Part of **MiniFuse for Linux**, an independent community project for Arturia
MiniFuse audio interfaces. Most users should start with
[MiniFuse Control](https://github.com/srdiogo/minifuse-control), the full mixer
application. This repository provides its driver, shared control library and
one service running in the logged-in user's session.

[Install](#install) · [Compatibility](#compatibility) · [Documentation](#documentation) · [Get help](SUPPORT.md)

## What it provides

- Shared hardware state for the full application and optional Omarchy panel.
- MiniFuse 4 mixer, routing, input-mode and phantom-power controls with readback.
- Calibrated live signal meters and host-side presets.
- Serialized control operations, capability checks and retained recovery records.
- A read-only `minifuse-doctor` command for checking an installation.

Normal USB audio continues to use `snd-usb-audio`. The control driver works
alongside it. The shared `mf-core` library is compiled into the service; the CLI
is not required, and Qt is needed only by the separate application.

## Install

**Current installation target: Arch Linux x86_64, with optional Omarchy integration.**
This is a development preview with bounded MiniFuse 4 hardware validation.

Download the archive and its checksum file from the
[complete installation preview](https://github.com/srdiogo/minifuse-backend/releases/tag/installer-preview-2026-10-29).
From the download directory:

```sh
sha256sum -c minifuse-linux-installer-2026-10-29.tar.gz.sha256
tar -xzf minifuse-linux-installer-2026-10-29.tar.gz
cd minifuse-linux-installer-2026-10-29
./install.sh
```

Run as your normal graphical-session user. The installer verifies the kit and
shows its plan before requesting confirmation. It prepares dependencies,
matching kernel headers, native packages and boot/login startup. The default
installs the application and adds the panel when Omarchy is present.

| What you want | Command inside the extracted kit |
|---|---|
| Application and backend, without the panel | `./install.sh --profile gui` |
| Application, backend and Omarchy panel | `./install.sh --profile full` |
| Backend and Omarchy panel, without the application | `./install.sh --profile omarchy` |
| Driver and service only | `./install.sh --profile backend` |
| Inspect the default plan without changes | `./install.sh --dry-run` |

The published GUI packages target Python 3.14. Existing installations and custom
kernels may need the options in the [installer guide](distribution/INSTALLER.md).
For manual package builds, startup, updates and removal, use
[the installation guide](docs/INSTALL.md). Keep the downloaded archive and
checksum together; a checksum checks integrity against that published file.

## Compatibility

| Area | Current status |
|---|---|
| MiniFuse 4 | Mixer, routes, input switches and meters have bounded hardware validation |
| MiniFuse 1 / 2 / 2 OTG | Automatic model layouts; experimental INST, +48 V and direct monitor/mono controls, disabled by default pending hardware tests |
| Arch Linux x86_64 | Native packages and complete installation kit available |
| Omarchy | Optional panel; tested host versions are recorded in the desktop source lock |
| Ubuntu / Debian / Fedora | Native distribution packages planned; current installer does not support them |
| Other desktop environments | Application has no Omarchy dependency; wider session acceptance remains open |

**Model-support test branch:** see [model-specific controls](docs/models/README.md),
[static analysis evidence](docs/models/EVIDENCE.md) and the
[hardware acceptance guide](docs/models/TESTING.md). The installation preview
linked above predates this feature. We welcome MF1, MF2 and MF2 OTG owners
who can provide readbacks, LED checks and minimal Windows USB captures.

An operator confirmed immediate and post-reboot operation of the installation
kit. The report did not identify a clean-machine versus upgrade setup; see
[installation evidence](docs/INSTALLATION_VALIDATION.md). Other hardware,
kernels and desktop combinations need their own validation.

Physical-logo events, effects, firmware changes and device-storage writes are
outside the available feature set. Presets are files on the computer and are
restored only by explicit user action.

## The projects

| Project | Purpose |
|---|---|
| [MiniFuse Control](https://github.com/srdiogo/minifuse-control) | Full mixer, routing, input controls and presets |
| **MiniFuse Backend** — this repository | Driver, shared core, session API and complete installer |
| [MiniFuse for Omarchy](https://github.com/srdiogo/omarchy-minifuse) | Optional bar indicator and compact mixer |

The backend repository was previously named `mf-service`. Installed package,
service, API and data-path names remain compatible. See
[project names and compatibility](docs/PROJECT_NAMES.md).

## Documentation

- [Installation, diagnostics and removal](docs/INSTALL.md)
- [Complete installer profiles and recovery](distribution/INSTALLER.md)
- [Distribution support and packaging roadmap](docs/DISTRIBUTION.md)
- [Reproducing and validating an installation](docs/REPRODUCING.md)
- [Hardware validation](docs/MF4_VALIDATION.md)
- [Development and architecture](docs/DEVELOPMENT.md)
- [API v1](docs/IPC.md), [live controls](docs/LIVE_CONTROLS.md) and [shared core](docs/CORE.md)
- [Changelog](CHANGELOG.md), [support](SUPPORT.md) and [contributing](CONTRIBUTING.md)

## Credits and license

This is an independent community project, not an official Arturia product.
Original contributors and protocol research are credited in
[the evidence record](docs/mf4/evidence.md) and [third-party notices](THIRD_PARTY.md).
New service code and documentation use the [MIT license](LICENSE); the control
driver uses [GPL-2.0-only](kmod/LICENSE). Component licenses remain separate.
