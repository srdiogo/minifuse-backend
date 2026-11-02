# Experimental model software validation

Recorded on 2026-11-01. Physical acceptance remains pending separately for MF1,
MF2 and MF2 OTG. This report describes software/static checks, not hardware
captures, real-device completion lengths or measured audio behavior.

| Check | Result |
|---|---|
| Rust 1.85.1 workspace | 73 tests passed, including five basic/native-model service scenarios |
| C/Rust protocol agreement | Passed: old and additive ioctl sizes/offsets, selector/value decisions and every 16-bit firmware descriptor across normal, initialization and unsupported model IDs |
| Firmware policy | Missing/malformed/old/unreviewed versions blocked; model-specific families, descriptor agreement and stale-plan invalidation tested |
| Native core | Synthetic gain/pan/mute/master/routing, model source groups, opaque-byte preservation, scoped snapshot restore and meter decoder checked |
| Native service | Continuous controls, preset capture/restore planning, meter events, module gates and firmware-change invalidation checked with synthetic hardware |
| Clippy, all targets, warnings denied | Passed |
| Installer/package/diagnostic tests | 20 passed |
| Native driver, `W=1`, kernel 7.2.8-9-omarchy headers | Built; module not loaded |
| Release service build | Passed |
| Temporary-prefix service install, unit parsing, isolated SIGTERM cleanup and removal | Passed |
| GUI, Python 3.14 / Qt 6.12.0, Basic and Fusion | 36 tests passed per style; QML lint passed |
| GUI native layouts | All three models rendered; stable faders on readback, firmware agreement, typed routing and meter shapes/expiry checked |
| Panel Python client | 34 tests passed |
| Panel host, Omarchy reference c668141 / Quickshell 0.3.1 / Qt 6.11.2 | 12 isolated construction scenarios plus pointer/focus interaction checks passed; QML lint passed |
| Visual inspection | Synthetic OTG application and panel renders inspected; routing labels and source/mute/fader alignment checked |

The GUI runner emits existing layout/teardown warnings while assertions pass;
these checks do not claim a warning-free runtime. Native fixtures are generated
by `loopback_fixture` without a device/socket. The `basic_status` read-only
collector was compiled and checked; it was not run against an MF1/MF2/OTG.
No installed application, running service, driver binding or audio setting was
changed. No firmware operation was performed.

An additional earlier panel run with Quickshell 0.3.2 / Qt 6.12.0 reported
undefined host color roles, also present in the MF4 baseline. That runtime is not
claimed as accepted. The supported panel baseline was tested using isolated
cached packages without changing the system. Wider host support remains open.

The expanded REA/Ghidra function analyses completed with Node 26.12.1. Their
interactive queue later reached its idle deadline; the overall runner did not
exit cleanly. The individual results and unchanged target hash are documented
in [evidence](EVIDENCE.md), without distributing proprietary inputs/results.

Follow [hardware acceptance](TESTING.md) before promoting any model or firmware
family. In particular, USB capture, actual transfer timing, selector behavior,
OTG second-host routing and meter channel/scale calibration remain unverified.
GitHub Actions validates exact PR heads and additional Rust/Python/package
combinations; a green build does not change the experimental hardware status.
