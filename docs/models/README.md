# Experimental MiniFuse 1, 2 and 2 OTG support

These models have their own input controls and **native hardware loopback mixer**.
Inventory selects the correct layout automatically. Reads and writes require a
deliberately enabled test session; physical acceptance is still pending for each
model. The normal installer does not enable experimental controls.

| Interface | Normal USB ID | B2 bytes | Candidate firmware family | Native loopback sources |
|---|---|---:|---|---|
| MiniFuse 1 | `1c75:af80` | 46 | `1.5.x` | Analog 1, USB 1–2, USB 3–4 |
| MiniFuse 2 | `1c75:af90` | 46 | `1.5.x` | Analog 1, Analog 2, USB 1–2, USB 3–4 |
| MiniFuse 2 OTG | `1c75:af93` | 52 | `1.0.x` | Analog 1, Analog 2, USB 1–2, USB 5–6 |
| MiniFuse 4 | `1c75:af70` | 190 | Existing independent implementation | Existing three-mix layout |

Initialization/storage IDs `af82`, `af92` and `af72` select an information-only
page. They never enter audio control transactions. Firmware and USB mode are
never changed by this software.

## Available controls

- Model-specific INST and +48 V switches; MF1 has no INST2. +48 V is shared on
  two-input models. Direct monitoring is labeled direct monitor on MF1 and direct
  mono on MF2/OTG. Physical input-gain and output knobs remain physical controls.
- One hardware loopback mix, source gain and mute, analog-source pan, and a
  stereo master. USB playback pairs have linked gain/mute and no pan or solo.
- Independent loopback-enable, duplicate recording on USB inputs 1–2, and
  physical-output/headphone monitoring flags. These are separate switches,
  not the MF4 route selector or membership controls.
- OTG-only Main → OTG selection, OTG → monitoring selection and front-panel
  meter-source selection. No unverified OTG-source gain fader is synthesized.
- Input and loopback-mix meters; OTG additionally reports an OTG input pair.
  USB playback strips have no individual native peak samples. Channel order and
  conversion follow the inspected host code; physical calibration is pending.
- Host presets with comparison before restore. Presets are model-specific and
  restore mapped mixer/routing fields only. INST, +48 V, direct monitoring and
  opaque words are preserved. They do not establish saving in device memory.

The full application provides input controls, mixer, routing, meters and presets.
The Omarchy panel provides native mixer/routing controls and meters, read-only
input indicators, and a shortcut to the full application for input changes and
preset management. Stable fader delegates retain pointer gestures across readback.

Arturia distinguishes the native mixer introduced with [MF1/MF2 firmware 1.5](https://www.arturia.com/products/audio/minifuse/update)
from **SLS**, its Windows-only host processing with additional virtual devices
and independent software mixes. This implementation controls the native mixer;
it does not reproduce SLS. The [OTG download catalog](https://www.arturia.com/support/downloads-manuals/product/minifuse-2-otg)
lists a separate 1.0.0 firmware line.

## Firmware compatibility

Inventory reads the cached standard USB `bcdDevice` descriptor from Linux sysfs.
The official Windows application uses that same descriptor for its firmware
version display. The decoder is `major = raw >> 8`, `minor = (raw >> 4) & 15`,
`patch = raw & 15`; for example `0152` is `1.5.2`.

The allowlist is intentionally family-specific: MF1/MF2 `1.5.0`–`1.5.9`, OTG
`1.0.0`–`1.0.9`. Missing, malformed, older or unreviewed newer families leave
loopback controls unavailable. **This is an experimental compatibility policy,
not a claim that every patch has been tested.** It is not a generic `>= 1.5`
comparison, and the shared resource's generic minimum `1.2.0` is not used as a
loopback threshold. A compatible version alone never grants writes.

`MF_SERVICE_EXPERIMENTAL_BASIC=1` enables only the existing experimental input
path. `MF_SERVICE_EXPERIMENTAL_LOOPBACK=1` enables that input path and, when
firmware, full state and the matching module agree, the native mixer. Missing
module capabilities leave those features unavailable. An unsupported firmware
may still expose basic switches if its exact bounded state can be read; no MF4
layout is substituted.

The driver rechecks PID, bus/address, **exact firmware descriptor**, and all
46/52 state bytes before loopback writes. Reconnection or firmware/capability
changes invalidate old plans and gestures. No firmware updater is included.

## Protocol and transaction boundaries

The model state is independent of MF4. Boolean input selectors are `0000` for
INST1 (word 0, bit 0), `0001` for INST2 (word 0, bit 1), `0400` for phantom
(word 4, bit 0), and `0500` for direct monitor/mono (word 5, bit 0).
[The protocol evidence](EVIDENCE.md) documents the additional selectors and meter
layout with model-specific bindings.

The basic driver ABI remains 68-byte status / 136-byte transaction, maximum four
switch writes. The additive firmware-gated ABI uses a 250-byte transaction,
maximum 32 changes, and a 44-byte meter record. Capability bits are `4` (basic
switches), `8` (native loopback transactions) and `16` (model-specific meters).
Multiple normal-mode basic interfaces are refused rather than guessed.

Every mutation uses the existing typed plan/apply or continuous-control workflow.
The core durably stores a private baseline and prepared journal before dispatch.
The driver preserves unknown bits/words, verifies the complete predicted state
and attempts bounded reverse rollback on failure. Restored is reported only when
complete readback equals the original state. No uncertain operation is replayed
after disconnect/restart. A no-op sends no writes and creates no journal.

`ModelProfile` is versioned presentation metadata, not permission. The static
catalog remains conservative (`inputs_and_monitoring`). Runtime inventory uses
`loopback_mixer` only when the advanced state is available. Unknown profiles stay
disabled. See [IPC](../IPC.md), [software validation](VALIDATION.md) and the
[hardware acceptance checklist](TESTING.md).

## Frontend development without hardware

[loopback-v1.json](loopback-v1.json) is a **synthetic** fixture generated by:

```sh
cargo run --locked -p mf-core --example loopback_fixture > docs/models/loopback-v1.json
```

The generator opens no device or socket. Matching copies in the GUI and panel
exercise per-model layouts, firmware agreement, meters and readback behavior.
Neither these fixtures nor successful software tests establish hardware support.
We welcome owners of MF1, MF2 and MF2 OTG to provide the checklist results.
