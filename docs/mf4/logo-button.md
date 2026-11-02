# Arturia logo short-press events

## Supported device

Runtime support is enabled only for a normal-mode MiniFuse 4 (`1c75:af70`)
whose USB `bcdDevice` is `0x0101` (displayed as 1.0.1), with a successful B2
state read and a counter in 0–9. This descriptor does not expose a fourth
firmware build component. Other firmware versions, initialization mode,
MiniFuse 1, MiniFuse 2 and MiniFuse OTG remain unsupported for this event.
A static model catalog is not sufficient to enable the capability.

No driver ABI change is required. The existing read-only B2 transport supplies
the 190-byte state. `State::control_panel_counter` decodes the little-endian
word at payload offset 14; this is a wrapping counter, not an on/off control.
Neither the service nor its agent writes it.

## Evidence

On 2 November 2026 an operator performed three separate short presses on one
MiniFuse 4. Read-only observations at approximately 100 ms intervals found:

| Observation | Counter | Changed payload bytes |
| --- | ---: | --- |
| Initial baseline | 7 | None |
| First short release | 8 | 14 only |
| Second short release | 9 | 14 only |
| Third short release | 0 | 14 only |

All other payload bytes remained identical. The collector used read-only
opens and made no configuration-write attempts. The sanitized evidence index
contains the private capture digest, timings and source identities; it excludes
raw device configuration, serial numbers and audio.

Static inspection provides independent context. The official Windows agent's
`ABUTTON` field is enum index 7. Its device controller establishes a baseline,
compares later values, and either launches MiniFuse Control Center or presents
its existing window (`RVA 0x433aa0` and `0x433d30`). Earlier static analysis of
MF4 firmware 1.0.1.35 found a short-release handler at `0x6514` incrementing the
word at offset 14 modulo 10. These are derived protocol facts; proprietary
executables, firmware images and decompiled sources are not redistributed.

## Event semantics

The first valid read establishes a baseline and never activates a window.
Each subsequent observed change produces one `control_panel_requested` event,
including 9 to 0. A random event ID identifies the observation independently
of the wrapping hardware counter. Repeated identical reads produce no event.
Several presses between polls are coalesced into one observed change, not
replayed as a guessed number of launches. Ten presses between observations
can wrap to the same value and cannot be detected by this transport.

The baseline is discarded after read/inventory failure, invalid counter,
reconnection or firmware/identity change. After a polling gap longer than two
seconds the next observation is also baseline-only. When a matching device has
an event subscriber, status polling runs at a nominal 200 ms interval; otherwise
normal service polling applies. Blocking transactions can delay observation.
This is a best-effort desktop affordance, not an input-event timing guarantee.

Events contain device identity, generation, a unique event ID and the source
and gesture labels. They carry no executable or desktop-specific target.
The optional desktop agent owns target selection, launch deduplication and
single-instance presentation. Starting the service or reconnecting an agent
never replays a previous press.

The complete state still participates in transaction verification. Unknown
header fields and the counter are preserved. A physical state change can
invalidate a pending transaction just as it did before this feature.

## Validation limits

Automated tests cover nonzero startup, unchanged samples, wrapping IDs,
firmware/counter rejection, read failure/recovery, connection replacement,
stale observations, subscription sequencing and zero hardware writes. Agent
socket tests cover baseline negotiation, event-ID deduplication, epoch/sequence
gaps and capability loss. Desktop acceptance is recorded separately below.

Long-press Sleep remains device-owned. It was not physically exercised in this
session, and no Sleep command, nonvolatile save or firmware modification was
introduced. Other models and firmware require their own evidence before
support is enabled.
