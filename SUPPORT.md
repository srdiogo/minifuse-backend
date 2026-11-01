# Getting help with MiniFuse Backend

Start with [installation](docs/INSTALL.md) and the compatibility table in the
[README](README.md#compatibility). The complete installer currently targets
Arch Linux x86_64. Ubuntu, Debian and Fedora delivery is planned; building a
component on another distribution does not establish complete support.

## Common problems

| Symptom | First check |
|---|---|
| Device or controls unavailable | Run `minifuse-doctor` as the graphical-session user; check the actual model and reported capabilities |
| Service unavailable | Inspect `systemctl --user status mf-service.service`; the service must belong to your login session |
| Driver not available for the kernel | Compare the running kernel, matching headers and `dkms status` |
| Application or panel cannot connect | Open Device and desktop diagnostics in the app; check backend/API compatibility |
| Missing panel or full-mixer launcher | Check the optional plugin/application installation separately |

Do not run a frontend or service as root to work around session access.
Package installation and startup configuration are described in the backend's
[installation guide](https://github.com/srdiogo/minifuse-backend/blob/main/docs/INSTALL.md).

## Where to report

| Project | Report scope |
|---|---|
| [MiniFuse Control](https://github.com/srdiogo/minifuse-control/issues/new/choose) | Application layout, presets, controls and launch |
| [MiniFuse Backend](https://github.com/srdiogo/minifuse-backend/issues/new/choose) | Installation, driver, service and hardware/API behavior |
| [MiniFuse for Omarchy](https://github.com/srdiogo/omarchy-minifuse/issues/new/choose) | Panel and Omarchy integration |

This repository covers: Driver, session service, protocol, installer and distribution packages. If unsure, open one report with the visible
symptom; maintainers can help route it. Search existing issues first.

Use the **Bug report**, **Installation help** or **Compatibility report** form
in [the issue chooser](https://github.com/srdiogo/minifuse-backend/issues/new/choose).
Include the installation method, component versions, distribution, kernel,
MiniFuse model and steps to reproduce. A screenshot is useful for presentation
problems. Compatibility reports should distinguish clean installation from
upgrade and identify exactly which features were checked.

## Sharing diagnostics

`minifuse-doctor --json` checks setup and API inventory without changing mixer
settings or printing device identities. Review any diagnostics before sharing.
For relevant service logs, inspect `journalctl --user -u mf-service.service`
locally and include only the redacted lines needed to explain the problem.

Keep serials, private paths, client tokens, operation identities, snapshots,
user presets, raw USB captures and microphone audio out of public issues.
Preserve unresolved operation records locally; deleting them is not a recovery
procedure. Do not toggle phantom power or send test tones just to complete a
support form.

This is a community preview. Reports with exact versions and bounded observations
help establish support for more machines; no response-time guarantee is offered.
