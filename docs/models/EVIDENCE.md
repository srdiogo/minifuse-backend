# Static host analysis

The reference is Arturia MiniFuse Control Center **1.4.2.1381**, x64 Windows PE.
Its SHA-256 is
`e2d8335a67b3be7bbaaf7952d53282e084a10d7d50c28ec4ca529b2e02cf106d`.
The image base is `0x140000000`. Addresses below are RVAs unless specified.
[evidence.json](evidence.json) records source/resource hashes, short byte witnesses
and private analysis-result hashes. Proprietary executables, resource XML, full
decompilation and original captures are not distributed with this project.

## Observed facts

- Each official device resource has an explicit normal USB VID/PID and separate
  initialization resources. The recorded normal IDs are `af80`, `af90`, `af93`
  and `af70`; initialization IDs are `af82`, `af92` and `af72` under VID `1c75`.
  The USB tag was read directly; the complete XML was not treated as valid XML
  because some resource files contain duplicate attributes.
- Model constructors start at `0x485f10` (MF1), `0x48d8b0` (MF2), and `0x495a40`
  (MF2 OTG). Generated parameter names and constructor bindings distinguish
  INST input index 0/1, phantom enum 4/index 0 and monitor enum 5/index 0.
- MF1/MF2 device vtable slot 16 points to `0x708c30`, which returns `0x17` words.
  MF2 OTG's corresponding slot points to `0x71f290`, which returns `0x1a` words.
- The shared read path at `0x48c9b0` sets request `b2` at `0x48ca12`, calls slot
  16 at `0x48ca27`, and doubles the returned word count at `0x48ca2d`.
- State dispatch at `0x48c110` indexes the word buffer by parameter enum times
  two (`0x48c1e7`). Per-input Boolean bindings select their bit within that word.
- The earlier MF1/MF2 allowlist already used selectors `0000`, `0400` and `0500`.
  MF2 input index 1 is an independently identified binding, not an MF4 assumption.

## Derived implementation

Those observations support candidate B2 lengths of **46, 46 and 52 bytes** and
the bounded Boolean selector/offset table in [the model guide](README.md).
The implementation uses class/interface control transfers, B2 reads and the
existing `22` parameter write transport. Exact on-device completion lengths,
field masks, response timing and audio behavior still require per-model capture.
The experimental gate is retained for that reason.

Resource names alone were insufficient to expose a mixer or calibrated meters.
The expanded investigation below follows the actual parameter and decoder
bindings. It supports an experimental native implementation, while physical
channel, scale and audio-path acceptance remains outstanding.

## Analysis procedure

REA 6.8.0 was installed in an isolated analysis directory with lifecycle scripts
disabled. Ghidra 12.1.4 used Java 21.0.13+9 and a 2048 MiB heap; the
`WindowsResourceReference` analyzer was disabled. `open_binary`, `binary_session`
and `analyze_function` completed on 2026-11-01, including an MF4 reference path
and each model's constructor/state presentation function. Completion was recorded
at `19:41:05.400Z`. Input hashes matched before and after analysis.

The host used Node v26.7.0, below REA's declared engine range. Actual successful
analysis is recorded independently of runtime support claims. Some decompiler
functions reported type-propagation warnings; the findings rely on constructor
bindings, RTTI/vtable targets and assembly/byte cross-checks, not inferred C types.

No target program was executed, no Windows VM was required for this static run,
and no hardware controls or firmware operations were performed. Physical-device
validation remains **pending for all three new models**.

## Native loopback: expanded investigation

A second static run used REA 6.8.0, Ghidra 12.1.4, Java 21.0.13+9 and **Node
26.12.1**, within REA's declared engine range. It started at
`2026-11-01T21:03:49.913Z`; the requested function analyses completed at about
`21:12:20Z`. The interactive queue was left open and its idle deadline closed
the session at `22:33:53.163Z`. The overall runner therefore recorded a deadline
failure; the completed individual results and unchanged source hashes remain
available as evidence. This is not described as a clean overall runner exit.

The additional constructor, vtable, transfer and decode paths establish more
than resource names. The following are derived **native hardware** mappings,
not the separate Windows SLS software engine. All require physical acceptance.

### Firmware descriptor path

Firmware UI `0x4575f0` invokes the descriptor closure at `0x71d980`. Its USB
interface dispatch reaches the libusb descriptor reader `0x73d520` or TUSB
adapter `0x728990`. The display reads the word at **USB descriptor offset 12**
(`bcdDevice`), then extracts the high byte and two low nibbles. Linux reads the
cached descriptor through sysfs; the kernel transaction checks its own cached
USB descriptor before any write. No vendor firmware query or update is needed.

The family policy combines those observations with Arturia's
[MF1/MF2 firmware 1.5 description](https://www.arturia.com/products/audio/minifuse/update)
and the [OTG 1.0.0 catalog entry](https://www.arturia.com/support/downloads-manuals/product/minifuse-2-otg).
The shared UI's generic minimum-version resource is not a model-specific
loopback compatibility test. New firmware families require review.

### Parameter bindings

Shared parameter dispatch `0x71c500` forms `(enum << 8) | index`. State dispatch
uses **enum × 2** as the byte offset. Native constructors at `0x485f10`,
`0x48d8b0` and `0x495a40`, together with shared level/pan binding `0x482940` and
stereo pair binding `0x48be40`, identify these fields:

| Function | Selector | Byte offset / encoding |
|---|---|---|
| Source left gain | `0600 + channel` | `2 × (6 + channel)`, signed Q8.8 dB |
| Source right gain | `0c00 + channel` | `2 × (12 + channel)`, signed Q8.8 dB |
| Source mute | `1200 + channel` | 36, bit `channel` |
| Loopback enable | `1300` | 38, bit 0 |
| Duplicate recording on USB 1–2 | `1400` | 40, bit 0 |
| Monitor mix on physical outputs/phones | `1500` | 42, bit 0 |
| Loopback master | `1600` | 44, signed Q8.8 dB |
| OTG → monitoring source | `1700` | 46, 0 main / 1 OTG; OTG model only |
| Front-panel meter source | `1800` | 48, 0 monitor out / 1 OTG in / 2 OTG out / 3 computer out |
| Main → OTG source | `1900` | 50, 0 OTG 3–4 / 1 output 1–2; OTG model only |

Channel 0 is Analog 1; channel 1 is Analog 2 and absent on MF1. Stereo playback
pairs use left channel 2/right channel 3 and left channel 4/right channel 5.
They are USB 1–2 and USB 3–4 on MF1/MF2; the second pair is **USB 5–6 on OTG**.
The unused cross-gain words are preserved, not overwritten by a paired fader.
The OTG resource also names an OTG-source fader without the corresponding direct
USB constructor binding; that is not enough evidence to expose a hardware gain.

Source control range is −60..+12 dB, master −64..0 dB; `8000` means silence.
Pan uses the shared equal-power level/pan conversion. Its attenuated individual
words may fall below the source fader's nominal minimum. There are no verified
native solo or membership selectors in this layout. Preset restore replays only
the mapped mixer fields and never input mode, phantom or direct-monitor state.

### Meter transfer and decoder

`0x488f20` requests class/interface IN **C2**, value/index zero. Device vtable
slot 17 returns four samples on MF1/MF2 and six on OTG; the transfer length is
sample count × four bytes. Decode functions are MF1 `0x488ad0`, MF2 `0x490ba0`
and OTG `0x4995f0`.

| Sample | Binding |
|---:|---|
| 0 | Analog input 1 |
| 1 | Analog input 2; ignored on MF1 |
| 2, 3 | Native loopback mix left/right |
| 4, 5 | OTG input left/right, OTG model only |

Each signed little-endian sample is shifted right by eight, then multiplied by
approximately `1/8388608`. The implementation converts that linear peak to dBFS;
zero is silence and negative magnitude is invalid. It does not label samples
2/3 as physical output meters or invent individual USB playback meters. The
host's initialization function also registers polling; that does not establish
a required USB initialization write. Only the fixed C2 read is implemented.

The source hash, result hashes and function addresses in [evidence.json](evidence.json)
are audit metadata. No proprietary executable, XML, full decompilation or firmware
image is included. Device captures, timing, exact completion lengths, audio-path
semantics and meter calibration remain open hardware acceptance items.
