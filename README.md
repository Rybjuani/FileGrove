# FileGrove 🌳📁

> Smart, simple file organization for everyone. Open-source desktop app
> that watches your folders and organizes files automatically based on
> rules you define.

## What is this?

FileGrove is a Linux-first desktop application that automates file
organization. The recovered Windows implementation remains a future
compatibility target, but the current reconstruction is developed and
validated on Linux. You add folders to watch,
define rules (by extension, name, size, date, regex), and FileGrove
moves new files into organized destinations — for example, sorting
your Downloads folder automatically, or filing photos by capture date.

It's a **clean-room reimplementation** inspired by
[File Arbor](https://github.com/furkanbagci34/file-arbor-releases) by
Furkan Bagci. No code was copied from the original; the implementation
is based on the behavioral specification in [`docs/SPEC.md`](docs/SPEC.md).

## Features

- Watch multiple folders simultaneously
- Rule engine with 9 condition types (extension, name contains/starts/ends, regex, size, age)
- Two clear folder modes: manual and automatic (real-time)
- Path templates with date and EXIF tokens (`{year}/{month}/{name}`)
- Recoverable system-trash actions and guarded permanent deletion
- Safe shell commands with test mode, timeout, cancellation, output capture,
  and change fingerprints that prevent accidental reruns
- Recursive subfolder scanning with excludes
- Durable Undo journal for moved files, including retained overwrite backups
  while the related Undo session remains available
- Activity history with command output, failures, and interrupted-action review
- System tray integration
- English and Spanish interface
- Single-instance desktop process with automatic-mode watchers
- **No telemetry, no license server, no paywall** — fully open-source

## Quick start

### Linux prerequisites

- [Rust](https://rustup.rs) 1.77.2+
- [Node.js](https://nodejs.org) 20+
- [Tauri 2 Linux prerequisites](https://tauri.app/start/prerequisites/)

### Install & run

```bash
npm install
npm run tauri dev
```

### Build for production

```bash
npm run tauri build
```

Output: `src-tauri/target/release/bundle/`

## Documentation

- [`docs/SPEC.md`](docs/SPEC.md) — Full behavioral specification
- [`docs/ROADMAP_1_0.md`](docs/ROADMAP_1_0.md) — Linux release gates through 1.0
- [`docs/HANDOFF_2026-08-08.md`](docs/HANDOFF_2026-08-08.md) — Exact continuation checklist and release blockers
- [`docs/INSTALL_LINUX.md`](docs/INSTALL_LINUX.md) — `.deb`, AppImage, runtime requirements, and current compatibility evidence
- [`docs/SAFE_USE.md`](docs/SAFE_USE.md) — Safe first run, Automatic mode, conflicts, and destructive actions
- [`docs/RECOVERY.md`](docs/RECOVERY.md) — Historical Undo, interrupted actions, Trash, and state backups
- [`docs/SECURITY.md`](docs/SECURITY.md) and [`docs/PRIVACY.md`](docs/PRIVACY.md) — Security boundaries and local-data policy
- [`docs/COMPETITIVE_POSITIONING.md`](docs/COMPETITIVE_POSITIONING.md) — Free capability parity target and FileGrove differentiators
- [`docs/CONTRIBUTING.md`](docs/CONTRIBUTING.md) — How to contribute

## Roadmap

The current v0.1.0 build is in local testing. Implemented scope includes:

- Folder watching + rule engine + manual/automatic organization
- Recursive scan, undo log, activity history
- Native Linux trash, EXIF tokens, safe command actions, and conflict handling
- Tray integration and single-instance startup
- English + Spanish localization

Before any public repository or release, the Linux build should receive sustained
local testing with disposable folders and a separate `.deb`/AppImage smoke test.
Windows work is intentionally on standby. Automatic updates are not implemented
during this local-testing phase.

## Contributing

Development is currently local. See
[`docs/CONTRIBUTING.md`](docs/CONTRIBUTING.md) for the project conventions.

## License

Apache License 2.0 — see [`LICENSE`](LICENSE).

## Credits

FileGrove is inspired by [File Arbor](https://github.com/furkanbagci34/file-arbor-releases)
by Furkan Bagci. See [`CREDITS.md`](CREDITS.md) for full credits.
