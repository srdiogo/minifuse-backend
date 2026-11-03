# Hardware acceptance: MiniFuse 1, 2 and 2 OTG

We are looking for owners of each model to validate the experimental native
loopback implementation. A Windows capture and Linux readback are useful even
if you cannot build the application. Test one model at a time; OTG requires its
own results. Builds and synthetic fixtures are not physical acceptance.

## Preparation

Use matching model-support revisions of backend, rebuilt driver and frontends.
The installation preview dated 2026-10-29 predates this work. See
[source installation](../INSTALL.md); do not mix an older module with the test
service. The source kit records exact commits and checksums, but does not bundle
all distribution dependencies or automatically install a new driver.

Connect one normal-mode interface. Record model/hardware revision, firmware as
shown by the official application, USB ID and `bcdDevice`, OS/kernel, source
commits and initial LEDs. Do not change firmware or USB mode. Decline firmware
update offers in the official application during capture. Keep speakers muted
for input-mode/phantom transitions; use equipment suitable for phantom power.
Use low levels and avoid microphone/speaker feedback for subsequent audio tests.

## Read-only Linux capture

Run the explicit collector from the backend checkout before allowing writes:

```sh
umask 077
MF_SERVICE_EXPERIMENTAL_BASIC=1 cargo run --locked -p mf-core --example basic_status > baseline.json
```

This opens the module read-only and performs one fixed B2 query. It reports
cached descriptor firmware, exact bounded payload and input state, with
`device_write_attempted: false`. Unsupported mode, ambiguous interfaces, wrong
length and an old module are errors; there is no fallback to an MF4 layout.

For a matching candidate firmware family and loopback-capable module:

```sh
MF_SERVICE_EXPERIMENTAL_LOOPBACK=1 cargo run --locked -p mf-core --example basic_status > mixer-baseline.json
MF_SERVICE_EXPERIMENTAL_LOOPBACK=1 cargo run --locked -p mf-core --example basic_status -- --meters > peaks.json
```

These commands still cannot send parameter writes. The second adds one fixed C2
read. Compare the displayed firmware with `firmware.version`, `raw_bcd` and the
family policy: MF1/MF2 1.5.x, OTG 1.0.x. Missing/malformed/older/unreviewed
families must not enable native mixer or meter reads. Do not update firmware as
part of this test; report the currently installed version.

Change **one physical button at a time** and capture off/on/off for INST1,
INST2 when present, +48 V and direct monitor/mono when exposed physically.
Record corresponding LEDs/audio behavior and restore the initial state. Compare
all payload bytes with the documented fields. Stop on unexpected lengths,
unrelated changes or contradictory indicators.

## Deliberate application session

Basic switches alone use `MF_SERVICE_EXPERIMENTAL_BASIC=1`. To test native
loopback, use a temporary session opt-in, with no second service on the device:

```sh
systemctl --user set-environment MF_SERVICE_EXPERIMENTAL_LOOPBACK=1
systemctl --user restart mf-service.service
```

Alternatively stop the installed service and run the test service in the
foreground with that variable. The normal installer sets neither variable.

1. Confirm model, firmware, exact sources and one native loopback mix. MF1 has no
   INST2/Analog 2. MF1/MF2 show USB 1–2 and 3–4; OTG shows USB 1–2 and 5–6.
   No MF4 output buses, solo, membership or stereo-pair pan may appear.
   Unknown/incompatible firmware must keep mixer, meters and presets unavailable.
2. Press supported physical buttons and confirm automatic application readback.
   Toggle each switch in the full application once each way; compare LED, full
   readback, operation outcome and audio. Test +48 V separately with speakers
   muted. Restore initial state after each off/on/off sequence.
3. With controlled low-level signals, test each source at −24, −12, 0 dB and
   silence; source range is −60..+12 dB. Test analog pan left/center/right,
   linked USB-pair gain/mute, and master −24, −6, 0 dB and silence. Faders apply
   while dragging, with final readback on release. Record selector/payload and
   audible/recorded result, not just the displayed value.
4. Test loopback enable, duplicate USB recording inputs 1–2 and monitor-mix flags
   **independently**. Confirm which recording channels and physical outputs are
   affected. Keep feedback paths controlled. These flags are not MF4 routes.
5. For OTG, attach a second host and test Main → OTG choices, OTG → monitoring,
   and each LED-meter-source choice separately. Record second-host routing and
   physical LEDs. There is no unverified OTG-source gain fader to test.
6. Inject one known channel at a time. Confirm input and loopback L/R meter
   ordering, silence, scale and changes with mute/master/routing. OTG also needs
   its own input pair checked. MF1's unused second input slot is ignored. Do not
   assume loopback meters are physical monitor meters. No individual USB playback
   peaks are advertised. Stale samples must clear after 500 ms without data.
7. Save a host preset, change mapped fields, compare and restore it. Verify exact
   full state and audio, with input switches/phantom/direct monitoring and opaque
   words unchanged. A preset from another model must fail before writing. Host
   presets must not be described as nonvolatile device saving.
8. Open the panel if used. Confirm native faders/routes/meters and read-only input
   indicators. Open the full application for input changes and presets. Capture
   layout/readback behavior without claiming a new host version is supported.
9. Reconnect, restart and switch models between sessions. Old plans/gestures must
   fail and no pending operation may replay. Verify default-disabled behavior
   after ending the opt-in. Report uncertain outcomes and preserve journals;
   never blindly retry an uncertain mutation.

End the session and restore the ordinary service environment:

```sh
systemctl --user unset-environment MF_SERVICE_EXPERIMENTAL_BASIC MF_SERVICE_EXPERIMENTAL_LOOPBACK
systemctl --user restart mf-service.service
```

Remove any separately configured permanent override too. Confirm experimental
controls return to unavailable. Restore the hardware's original state deliberately;
this software does not automatically restore phantom power or input mode.

## Windows reference capture

Use the official Control Center on Windows or with exclusive USB passthrough to
a VM. Capture only the interface with USBPcap/Wireshark. Record application/driver
versions, VID/PID, firmware descriptor and a timestamped action list. Close other
control applications and change only one field in each marked interval.

Capture startup and B2 polling, then supported button sequences, source gains,
analog pan, mute, master, each routing flag and C2 meters. For OTG capture all
three additional selectors and second-host connection conditions separately.
Preserve setup packets, completion lengths, payloads and timestamps. Separate
native hardware traffic from Windows SLS host/virtual-device behavior. Do not
capture firmware updates, mode transitions, effects or device-storage operations.
A descriptor-only capture cannot validate parameter payloads.

## Reporting and promotion

Report model/firmware and pass/fail/not tested per control, LED, full payload,
operation outcome, audio path and meter channel/scale. Share minimal readbacks,
relevant sanitized capture excerpts and journals after checking for unrelated
identifiers. Never attach proprietary software or firmware images.

Promotion is model-specific and requires reviewed captures, repeatable writes,
readback, rollback/conflict behavior where feasible, reconnect/restart results
and firmware agreement. Static analysis does not replace that evidence.
