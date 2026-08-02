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

### Local-testing status

- The interface currently ships in English and Spanish
- Automatic updates are intentionally excluded
- Installer smoke testing and sustained real-folder testing remain before a
  public release

### Removed

- Scheduled/cron organization from the Linux-first product. Legacy scheduled
  folders migrate to inactive manual mode while their old schedule data remains
  preserved for compatibility.
