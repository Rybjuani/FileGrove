# FileGrove 🌳📁

> Smart, simple file organization for everyone. Open-source desktop app
> that watches your folders and organizes files automatically based on
> rules you define.

## What is this?

FileGrove is a cross-platform desktop application that automates file
organization. The current `0.1.0` local test build is produced and under test
on Windows 10/11 x64 (NSIS installer); Linux packages exist from an earlier
build, and full clean-machine Linux validation remains in progress. You add
folders to watch, define rules (by extension, name, size, date, regex), and
FileGrove moves new files into organized destinations — for example, sorting
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
- Safe duplicate finder: size grouping plus SHA-256, oldest copy kept as
  the original, selected duplicates sent only to the system Trash/Recycle Bin
- Repository-aware scanning: `.git`, `.hg`, and `.svn` trees are excluded,
  and repository roots cannot be added as watched folders
- Loop protection: managed destinations are excluded, files already in
  their destination are skipped, and new folders scan only their root by default
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

### Windows local test build

The current Windows artifact is a per-user NSIS installer:

```text
src-tauri/target/release/bundle/nsis/FileGrove_0.1.0_x64-setup.exe
```

It installs without administrator rights and uses Windows WebView2. See
[`docs/TESTING_WINDOWS.md`](docs/TESTING_WINDOWS.md) for the installer hash,
first-run behavior, test-data guidance, and the suggested test checklist.

For development on Windows:

- [Rust](https://rustup.rs) 1.77.2+
- [Node.js](https://nodejs.org) 20+
- [Tauri 2 Windows prerequisites](https://tauri.app/start/prerequisites/)
- Microsoft Edge WebView2 Runtime (normally included with Windows 10/11)

```powershell
npm install
npm run tauri dev
```

### Linux prerequisites

- [Rust](https://rustup.rs) 1.77.2+
- [Node.js](https://nodejs.org) 20+
- [Tauri 2 Linux prerequisites](https://tauri.app/start/prerequisites/)

```bash
npm install
npm run tauri dev
```

### Build for production

```bash
npm run tauri build
```

Output: `src-tauri/target/release/bundle/`

### Canonical launcher

From the repository root, use `filegrove.cmd` on Windows or `./filegrove.sh` on
Linux. They run the bundled release binary when it exists, and fall back to the
development launcher (`npm run tauri:dev`) otherwise. Pass `--dev` to force the
development launcher. Rebuild the release binary after source changes, or the
launcher will keep starting the previous build.

The first run creates the conservative personal profile described in
[`docs/LAUNCH.md`](docs/LAUNCH.md). It watches Desktop and Downloads in
Automatic mode and enables launch at startup; pause it or use a separate test
account before pointing it at real files.

## Documentation

- [`docs/SPEC.md`](docs/SPEC.md) — Full behavioral specification
- [`docs/TESTING_WINDOWS.md`](docs/TESTING_WINDOWS.md) — Windows 0.1.0 test build, first-run behavior, and test checklist
- [`docs/ROADMAP_1_0.md`](docs/ROADMAP_1_0.md) — Linux release gates through 1.0
- [`docs/HANDOFF_2026-08-08.md`](docs/HANDOFF_2026-08-08.md) — Historical continuation checklist and Linux release blockers
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
- Native Windows Recycle Bin / Linux Trash, EXIF tokens, safe command actions,
  and conflict handling
- Safe duplicate finder with SHA-256 verification and Trash-only cleanup
- Repository-aware scanning and managed-destination loop protection
- Tray integration, single-instance startup, and a per-user Windows installer
- English + Spanish localization

The current phase is sustained local testing on Windows with disposable copies.
Linux clean-machine package validation remains an open gate. Automatic updates
are not implemented during this local-testing phase.

## Contributing

Development is currently local. See
[`docs/CONTRIBUTING.md`](docs/CONTRIBUTING.md) for the project conventions.

## License

Apache License 2.0 — see [`LICENSE`](LICENSE).

## Credits

FileGrove is inspired by [File Arbor](https://github.com/furkanbagci34/file-arbor-releases)
by Furkan Bagci. See [`CREDITS.md`](CREDITS.md) for full credits.
