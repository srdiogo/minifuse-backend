# Model-controls software validation

Recorded on 2026-11-01. MF1/MF2/OTG hardware acceptance remains pending.

| Check | Result |
|---|---|
| Rust 1.85.1 workspace tests | 65 passed, including 3 basic-model service scenarios |
| C/Rust ABI and allowlist comparison | Passed; fixed 68-byte status and 136-byte transaction records |
| Clippy, all targets, warnings denied | Passed |
| Installer/package/diagnostic tests | Passed |
| Native driver, `W=1`, kernel 7.2.8-9-omarchy headers | Built; module was not loaded |
| Release service build | Passed |
| Temporary-prefix service install, unit parsing, SIGTERM cleanup and removal | Passed |
| GUI, Python 3.14 / Qt 6.12.0, Basic and Fusion styles | 29 tests passed per style; QML lint passed |
| GUI wheel resources and temporary install/removal without Qt | Passed; includes ModelPage.qml |
| Panel client tests | 32 passed |
| Panel host baseline, Omarchy reference c668141 / Quickshell 0.3.1 / Qt 6.11.2 | QML lint, 9 construction scenarios and pointer interaction checks passed |

The presentation fixture is synthetic. New-model tests do not replay a real
MF1/MF2/OTG USB capture. The status capture example was compiled and checked;
it was not run against a physical interface. No installed application, running
service, driver binding or audio setting was changed during this work.

The GUI test runner emitted layout/teardown warnings while its assertions passed;
these results do not claim warning-free runtime acceptance. Visual inspection
covered the basic model page at the minimum supported window size.

An additional panel run with the machine's newer Quickshell 0.3.2 / Qt 6.12.0
reported undefined host color roles. The same failure occurred in the existing
MF4 construction scenario; it is not a successful acceptance result for that
runtime. The supported host baseline was tested separately using isolated cached
runtime packages, without changing the system. Wider host-version support still
requires investigation and must not be inferred from these model tests.

Follow [the hardware checklist](TESTING.md) before promoting any new model from
experimental status. GitHub Actions checks apply to the exact PR head and may
cover additional Python, Rust and packaging environments.
