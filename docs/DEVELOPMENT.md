# Backend development

The workspace contains `crates/mf-core` (typed device state, bounded operations
and host snapshots), `crates/mf-service` (coordination and API) and `kmod` (Linux
control transport). The core is compiled into one service process. Frontends
remain separate projects.

## Build and check

Requires Linux, Rust 1.85+, a C compiler with AddressSanitizer/UndefinedBehaviorSanitizer
for ABI/driver tests, Python 3.12+ for tooling,
systemd tools for distribution checks and the committed Cargo lockfile.

```sh
cargo build --release --locked
make check
make check-tools
make check-distribution
./target/release/mf-service --help
```

`make check` runs formatting, workspace tests and Clippy. Distribution checks
stage/remove files and exercise isolated socket/process cleanup. They do not
enable user startup or load a module. The driver fault-injection test compiles
the production model transaction header with in-memory kernel/USB hooks. It
exercises 122 rejection, partial-write, readback, rollback and lost-reply scenarios
without device access. It requires a normal untraced run for LeakSanitizer;
actual device behavior remains subject to model acceptance. Driver compilation
is separate:

```sh
make check-driver KDIR=/path/to/matching/kernel/build
```

The complete-kit installer is tested with fake hosts and temporary directories.
Never run an installation or a real-device write as a packaging test. Follow
[hardware validation](MF4_VALIDATION.md) for explicitly prepared hardware work.

## API and storage

The service uses `$XDG_RUNTIME_DIR/mf-service/control.sock`, checks peer UID and
refuses to replace a live instance. Its directory is mode 0700 and socket 0600.
It runs as the session user and never loads a driver or restores a preset on
startup. Normal device access requires an administrator-installed control driver.

Negotiate `hello`, obtain inventory and a confirmed state baseline, then use
the typed [API](IPC.md). Continuous fader/pan changes use
[live controls](LIVE_CONTROLS.md); discrete operations use plan/apply. Accepted
operations have durable identities. Reconnects and uncertain outcomes require
reconciliation rather than automatic replay.

Operation records live under `$XDG_STATE_HOME/mf-service`, historical core
backups under `$XDG_STATE_HOME/mf-cli`, and presets under
`$XDG_DATA_HOME/minifuse/presets`. Host snapshots do not establish nonvolatile
device saving. Meter samples are ephemeral and do not create state revisions
or journals. Input-switch changes require advertised driver support, and mixer
preset restore does not change input mode or phantom power.

See [architecture](ARCHITECTURE.md), [the shared core](CORE.md),
[implementation and acceptance](IMPLEMENTATION.md), [contributing](../CONTRIBUTING.md)
and [release preparation](RELEASING.md).
