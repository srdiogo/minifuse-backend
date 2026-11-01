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

The presence of loopback or software-loopback names in official resources does
not establish a device mixer. Meter-related native functions do not establish
channel order, scale or endpoint semantics. Neither is exposed for these models.

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
