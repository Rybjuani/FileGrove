# FileGrove

> Smart, simple file organization for everyone. Open-source desktop app
> that watches your folders and organizes files automatically based on
> rules you define.

[![License: Apache 2.0](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![CI](https://github.com/filegrove/filegrove/actions/workflows/ci.yml/badge.svg)](https://github.com/filegrove/filegrove/actions)
[![Release](https://img.shields.io/github/v/release/filegrove/filegrove.svg)](https://github.com/filegrove/filegrove/releases)

## What is this?

FileGrove is a cross-platform desktop application (Windows, macOS,
Linux) that automates file organization. You add folders to watch,
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
- Three folder modes: manual, automatic (real-time), scheduled (cron)
- Path templates with date and EXIF tokens (`{year}/{month}/{name}`)
- Recursive subfolder scanning with excludes
- Undo log — revert any organize operation
- Activity history with full audit trail
- System tray integration
- 8 languages (English, Spanish, French, German, Italian, Portuguese, Russian, Turkish)
- Cross-platform: Windows, macOS, Linux
- ~10 MB bundle (Tauri, not Electron)
- **No telemetry, no license server, no paywall** — fully open-source

## Quick start

### Prerequisites

- [Rust](https://rustup.rs) 1.75+
- [Node.js](https://nodejs.org) 20+
- [Tauri 2 prerequisites](https://tauri.app/v2/guides/getting-started/prerequisites/)

### Install & run

```bash
git clone https://github.com/filegrove/filegrove.git
cd filegrove
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
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — Architecture deep-dive
- [`docs/IPC.md`](docs/IPC.md) — IPC channel reference
- [`docs/RULES.md`](docs/RULES.md) — Rule engine semantics
- [`docs/CONTRIBUTING.md`](docs/CONTRIBUTING.md) — How to contribute

## Roadmap

See [Milestones](https://github.com/filegrove/filegrove/milestones) for
the current roadmap. v0.1.0 (MVP) targets:

- Folder watching + rule engine + manual/auto/scheduled organize
- Recursive scan, undo log, activity history
- Tray integration, onboarding tour, auto-update
- English + Spanish localization

Postponed to v0.2.0: EXIF-based sorting, run-command action.

## Contributing

Pull requests welcome! Please read
[`docs/CONTRIBUTING.md`](docs/CONTRIBUTING.md) first.

## License

Apache License 2.0 — see [`LICENSE`](LICENSE).

## Credits

FileGrove is inspired by [File Arbor](https://github.com/furkanbagci34/file-arbor-releases)
by Furkan Bagci. See [`CREDITS.md`](CREDITS.md) for full credits.
