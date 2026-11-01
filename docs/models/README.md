# Model-specific controls

MiniFuse 1, MiniFuse 2 and MiniFuse 2 OTG have experimental input and direct
monitoring support. Device detection and presentation are automatic. Hardware
reads and writes for these models require an explicitly enabled test session.
They have **not yet been validated on physical hardware**.

| Interface | Normal USB ID | B2 bytes | Implemented test controls | Hardware acceptance |
|---|---|---:|---|---|
| MiniFuse 1 | `1c75:af80` | 46 | INST 1, +48 V, direct monitor | Pending |
| MiniFuse 2 | `1c75:af90` | 46 | INST 1, INST 2, shared +48 V, direct mono | Pending |
| MiniFuse 2 OTG | `1c75:af93` | 52 | INST 1, INST 2, shared +48 V, direct mono | Pending separately |
| MiniFuse 4 | `1c75:af70` | 190 | Existing mixer, routing, input switches, meters and host presets | Bounded reference validation |

Initialization/storage IDs `af82`, `af92` and `af72` are recognized for
presentation only. They never enter audio control transactions. The application
does not change USB mode, firmware or device storage.

## Evidence and boundaries

The implementation combines the existing MF1/MF2 selector allowlist with
model-specific analysis of MiniFuse Control Center 1.4.2.1381. REA 6.8.0 and
Ghidra 12.1.4 completed static analysis of the three model constructors and
state presentation paths. The executable was not run and its hash was unchanged.
[Evidence](EVIDENCE.md) separates direct observations from protocol deductions.

The normal-mode state lengths are 23, 23 and 26 little-endian words. The
experimental switch mapping is:

| Control | Selector | Byte offset | Word mask | Models |
|---|---|---:|---:|---|
| INST 1 | `0000` | 0 | `0001` | MF1, MF2, MF2 OTG |
| INST 2 | `0001` | 0 | `0002` | MF2, MF2 OTG |
| Phantom power | `0400` | 8 | `0001` | MF1, MF2, MF2 OTG |
| Direct monitor / mono | `0500` | 10 | `0001` | MF1, MF2, MF2 OTG |

Only Boolean values are accepted. All other words and bits are preserved.
This mapping remains a test hypothesis until the [hardware checklist](TESTING.md)
passes on each model. MF4 uses its existing independent state and selectors;
in particular, its phantom-power selector is different.

No MF4 faders, mix routes, snapshots or meter calibration are applied to other
models. Physical gain knobs are not remote gain controls. Windows host software
loopback fields are not assumed to be hardware mixer fields. OTG routing and
meter endpoints require their own evidence and are not implemented here.

## Architecture

`ModelProfile` is presentation metadata, not a permission grant. Inventory always
includes it, even when the test transport is disabled. Profiles describe the
normal or initialization mode, physical I/O, layout, available input targets and
validation level. `get_model_profiles` exposes the same catalog without hardware.
[profiles-v1.json](profiles-v1.json) contains synthetic presentation fixtures.

`BasicState`, `BasicPlan` and the basic transport are independent of MF4's
190-byte layout. The driver uses fixed-size status/transaction records, a
model-specific length, a maximum of four switch changes and a strict selector
allowlist. It refuses multiple basic interfaces instead of choosing one.

The service requires `MF_SERVICE_EXPERIMENTAL_BASIC=1`, an exact identity match,
and module capability bit `4`. Older modules leave controls unavailable. The
application enables buttons only after confirmed state and advertised write
support. The Omarchy panel shows model-specific **read-only indicators** and
opens the full application for control.

Every write follows the existing plan/apply workflow. The service retains an
operation identity, and the core durably writes a baseline and prepared journal
before dispatch. The driver rechecks the entire model-specific state immediately
before writing. It verifies all 46/52 bytes after the transaction and attempts
bounded rollback on failure. Rollback is reported as verified only when complete
readback equals the original state. Reconnection invalidates plans. Nothing is
replayed automatically after uncertainty or restart.

These journals are diagnostic/recovery records, not user presets. No basic-model
snapshot restore is exposed. A no-op neither dispatches nor creates a journal.

## Development validation

Run the workspace tests and Clippy, the C/Rust protocol comparison, and a
build-only driver check. The protocol test checks every selector with Boolean
and invalid values across normal, initialization and unsupported model IDs.
Service tests use synthetic state for per-model dispatch, readback, reconnect,
disabled capability and initialization-mode gates. Frontend tests verify
per-model controls, read-only panel behavior and absence of MF4 mixer views.

Software tests are not substitutes for LED, audio and Windows USB capture
acceptance. Promote one model at a time only after its evidence is recorded.
