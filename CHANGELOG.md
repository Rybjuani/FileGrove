# Changelog

All notable changes to FileGrove will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Project scaffold: Tauri 2 + React 18 + TypeScript + Tailwind + Rust
- SQLite persistence with `refinery` migrations (V0001 initial schema)
- Domain types: `Folder`, `Rule`, `Condition`, `Action`, `Activity`,
  `UndoEntry`
- Rule engine: condition matcher, plan computation, action executor
  (safe move, Recycle Bin, guarded delete, and runCommand)
- Full path token resolver with local file dates and EXIF capture metadata,
  camera, lens, ISO, focal length, and dimensions
- Command test UI, process-tree timeout/cancellation, 1 MiB output cap,
  environment variables, and persistent anti-rerun fingerprints
- Cross-volume moves with explicit rename/overwrite/skip behavior
- Recursive scanner with depth, excludes, preserved relative structure,
  progress throttling, and a 100,000-file safety cap
- Secondary Windows drive roots as watched folders, with the system drive kept
  protected and drive metadata folders (`$RECYCLE.BIN`, `System Volume
  Information`, `Recovery`, `Config.Msi`, `.Trash-1000`, `SteamLibrary`)
  ignored by default
- Watcher service with functional pause/resume and startup restoration
- Cooperative organization and command cancellation, plus retryable undo
  sessions that do not overwrite existing files
- Tray icon with Open/Pause All/Resume All/Quit menu
- Single-instance protection and real launch-at-startup/tray settings
- IPC handlers for: app, folders, rules, files, organize, watcher,
  settings, activity, permissions
- React UI: Dashboard, Rules, History, Settings, About pages
- Zustand store (`useAppStore`) for global state
- i18n with EN + ES catalogs (ES extracted from File Arbor v1.1.2)
- Apache 2.0 license, NOTICE, CREDITS, CONTRIBUTING
- `docs/SPEC.md` — full behavioral specification
- `docs/M1_ISSUES.md` — local validation checklist
- Testable Undo core with disposable-filesystem regression coverage for
  cancellation and multiple historical sessions
- Black-box watcher integration tests for event bursts, pause/resume catch-up,
  and active or paused service restoration after restart
- Real recursive organization coverage for flattened and preserved layouts,
  maximum depth, and excluded folders
- Real-file safety matrix for rename, skip, overwrite, keep-both, Trash, and
  explicitly enabled permanent duplicate deletion
- Simplified custom-rule flow with an automatically suggested managed folder,
  safe defaults, and progressively disclosed advanced actions
- Persistent first-run welcome that explains the safe Manual-to-Automatic
  workflow, recommended categories, Preview, pause, and historical Undo
- Calmer dashboard hierarchy with wider folder cards, explicit primary and
  management actions, accessible progress feedback, and a dedicated Undo panel
- Keyboard and screen-reader accessibility pass with trapped/restored dialog
  focus, skip navigation, semantic table controls, visible focus, reduced
  motion support, complete control labels, and safer light-theme contrast
- Audited Linux 1.0 handoff separating current requirements from historical
  specification parity, with package, safety-test, documentation, and release checklists
- Linux release documentation for installation, safe operation, recovery,
  security, privacy, package metadata, compatibility evidence, and contributor workflow
- Canonical FileGrove tree mark shared by the interface and generated Linux,
  Windows, macOS, Android, and iOS icon assets
- Low-risk visual polish: 10/12/16 px shared surfaces, 44 px tactile controls,
  softer borders and shadows, coherent inputs and switches, and a wider
  single-column dashboard until the `xl` breakpoint
- Simplified Activity history into a compact file/action/result summary with
  technical paths and command details shown only on demand
- Safe duplicate finder for each watched folder: exact-size grouping followed
  by streaming SHA-256, the oldest copy marked as the original, and selected
  duplicates sent only to the system Trash/Recycle Bin
- Repository guardrails: `.git`, `.hg`, and `.svn` trees are pruned from scans,
  previews, watchers, and duplicate scans, and repository roots are rejected as
  watched folders
- Managed-destination loop protection across the scanner, organizer, and
  watcher; files already at their destination are skipped, and new folders scan
  only their root by default
- Protection for the application database/config directory and the executable
  directory as watched folders or destinations
- Per-user Windows NSIS installer and a standalone Windows release build

### Changed

- Canonical launchers now run the bundled release binary and fall back to
  `npm run tauri:dev`; they no longer start an isolated debug binary
- Windows path canonicalization uses `dunce` to avoid extended-length prefixes
- The duplicate dialog refreshes after a partial Trash failure, reports IPC
  failures, and uses translated headings and accessibility labels

### Local-testing status

- The interface currently ships in English and Spanish
- Automatic updates are intentionally excluded
- The Windows 0.1.0 NSIS installer is ready for multi-day local testing with
  disposable copies; it is unsigned, so Windows SmartScreen may warn
- Linux packages remain from the earlier build and still require clean-machine
  installation, upgrade, and AppImage validation
- Full release-candidate testing remains in progress

### Removed

- Scheduled/cron organization from the product. Legacy scheduled
  folders migrate to inactive manual mode while their old schedule data remains
  preserved for compatibility.
