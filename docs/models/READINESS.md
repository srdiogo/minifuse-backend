# Model support readiness

The implementation is ready for **experimental hardware acceptance**, with
software checks kept separate from physical evidence. Native loopback is available automatically with compatible firmware, confirmed state and matching driver capabilities. No new model is
promoted to physically validated status by passing a simulator or CI job.

| Area | Verified without a device | Remaining physical evidence |
|---|---|---|
| Model/firmware guards | PID/mode/length allowlists; cached descriptor parsing; unknown/old/unreviewed firmware rejection; exact firmware mismatch and stale-plan invalidation | Compare descriptor firmware with the official display on each interface |
| Native mixer/routing | Inspected host bindings, typed plans, model source groups, full-state preservation, live-control coordination | Correct selector effects and audible/recorded paths on each model |
| Driver transactions | Production C transaction header executed with in-memory kernel/USB hooks; rejection, partial writes, missing acknowledgements, full readback, bounded reverse rollback and lost replies | Real transfer completion lengths, device timing, USB disconnect/reconnect and firmware behavior |
| Input switches | Model-specific flags, strict typed changes and readback UI behavior | Physical button, LED, +48 V and direct-monitor/mono agreement |
| Meters | Static transfer/decoder mapping, bounded model shapes, numerical conversion, invalid records and stale-data expiry | Actual channel order, level calibration and relationship to routing/mute/master |
| OTG | Model-specific selectors, enum bounds, state/preset handling and synthetic UI interactions | Second-host connection and both directions of audio routing |
| Host presets | Same-model validation, mapped-field restore, stereo-side comparison and preservation of input/opaque state | Confirm complete readback and resulting audio on each interface |
| Desktop clients | Synthetic rendering, theme updates, real pointer events, confirmed-state handling and reconnect/recovery tests | Normal hardware use in a graphical session with the corresponding interface |

The driver harness does not emulate an Arturia firmware implementation. Its
in-memory state models the inspected mapping so it can test **the driver's own
control flow and reporting**, including acknowledgements lost after a write.
It cannot independently prove that the mapping matches a physical interface.
Address/undefined-behavior sanitizers and allocation/reference/lock accounting
cover the harness execution; they do not validate the Linux USB stack or real
bus timing. No device node or kernel module is opened by that test.

The panel's Qt 6.12 color-name collision is resolved using an explicit Omarchy
module qualifier. The pinned old/new runtime matrix and native pointer scenarios
are recorded in the panel validation report and source lock. This is isolated
software acceptance, not a claim of new device support.

Real captures supplied by another owner can be inspected offline and used as
additional evidence; local possession of every interface is not required. They
must still come from that model and firmware and include the relevant actions.
Audio recordings and LED observations must be reported by the owner. Synthetic
traffic cannot replace those observations.

See [software results](VALIDATION.md), [protocol evidence](EVIDENCE.md) and the
[physical checklist](TESTING.md). Windows SLS host processing, effects, firmware
changes and nonvolatile device-storage writes remain outside this native scope.
