# Hardware acceptance: MiniFuse 1, 2 and 2 OTG

We are looking for owners of each model to validate this experimental support.
A Windows capture and Linux readback are useful even if you cannot build the
application. Test one model at a time; MF2 OTG needs its own results.

## Preparation

Use the model-controls test revisions of the backend and frontends, with the
matching rebuilt driver for the running kernel. The installer preview dated
2026-10-29 predates this feature and does not contain it. Follow
[source installation](../INSTALL.md) for the test checkout; do not mix its module
with an older service and assume capability support.

Connect one normal-mode interface. Record model, hardware revision if available,
firmware version as reported by the official application, USB ID, OS/kernel,
module/service/application commits and initial LED states. Do not change firmware
or USB mode for this test. Keep speakers muted for input-mode/phantom transitions.
Only test +48 V with equipment suitable for phantom power.

## Read-only Linux capture

Before allowing application writes, capture a baseline with the explicit read-only
example from the backend checkout:

```sh
MF_SERVICE_EXPERIMENTAL_BASIC=1 cargo run --locked -p mf-core --example basic_status > baseline.json
```

This opens the module read-only and performs one fixed B2 query. It cannot send
parameter writes. It reports the complete bounded payload, decoded switches and
`device_write_attempted: false`. Unsupported mode, ambiguous interfaces, wrong
length or an old module are errors, not a fallback to another model's layout.

Change **one physical button at a time** and run the same command to another
file. Capture off/on/off for INST1, INST2 when present, phantom power and direct
monitor/mono when exposed physically. Record corresponding LEDs and audio
behavior. Restore the initial state after each sequence. Compare full payloads
against the candidate offsets/masks before testing application writes. Stop and
report an unexpected length, extra changed bits or contradictory indicators.

## Application write acceptance

Enable the experimental setting for a deliberate test session only. For a
systemd user service, a temporary session environment can be used:

```sh
systemctl --user set-environment MF_SERVICE_EXPERIMENTAL_BASIC=1
systemctl --user restart mf-service.service
```

Alternatively stop the installed service and run the test service in the
foreground with that environment variable. Do not run a second service against
the same socket/device. The variable is not set by the normal installer.

1. Start the full application. Confirm the correct model, input count and labels.
   MF1 must not have INST2. No MF4 faders, routing, meters or preset restore may
   appear for a basic model. Initialization mode must offer no audio controls.
2. Press each supported physical button. Confirm the application follows readback
   without a manual read button, including changes while the panel is open.
3. Toggle each switch in the application once in each direction. Confirm its LED,
   complete readback, verified operation/journal and expected audio behavior.
   Test phantom power separately, with speakers muted. Restore the initial state.
4. Confirm direct monitor (MF1) or mono summing (MF2/OTG) audibly using a controlled
   low-level input. Physical monitor-blend/output knobs remain physical controls.
5. Open the Omarchy panel if used. It must show this model's read-only indicators
   and open the full application. No MF4 mix or calibrated meter may be shown.
6. Reconnect the interface, restart the service and switch models between sessions.
   Old plans must fail, state must refresh, and no pending operation may replay.

End the opt-in session and restart the ordinary service:

```sh
systemctl --user unset-environment MF_SERVICE_EXPERIMENTAL_BASIC
systemctl --user restart mf-service.service
```

Confirm controls return to unavailable for the experimental model. A separately
configured permanent service override must also be removed if one was used.

## Windows reference capture

Use the official Control Center with the interface attached directly to Windows
or passed through exclusively to a VM. Capture that interface's USB traffic with
USBPcap/Wireshark. Record application/driver versions and the VID/PID. Close other
control applications and change only one control per marked time interval.

Capture startup/state polling, then off/on/off for INST1, INST2 when applicable,
phantom and direct monitoring/mono. Preserve setup packets, completion lengths,
payload bytes, timestamps and a text list of physical/GUI actions. No firmware
update, initialization transition, effects or device-storage operations belong
in this capture. A descriptor-only capture cannot validate control payloads.

For MF2 OTG, report the OTG connection state, but keep OTG routing discovery as a
separate capture; no routing writes are implemented in this test revision.

## Reporting and promotion

Share a result table listing each direction, observed LED, payload change,
operation outcome and audio result. Attach minimal relevant readbacks and
sanitized capture excerpts; check USB captures for unrelated device identifiers
before publishing. Never attach the proprietary application or firmware image.

Mark each step pass/fail/not tested. Report transfer failures and journals
without retrying uncertain writes automatically. A software-only fixture or
successful build must not be reported as hardware acceptance. Default enablement
requires reviewed captures, repeatable off/on/off behavior and reconnect/restart
results for that exact model. Unverified advanced features remain unavailable.
