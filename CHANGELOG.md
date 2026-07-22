# Changelog

All notable changes to FileGrove will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Project scaffold: Tauri 2 + React 18 + TypeScript + Tailwind + Rust
- SQLite persistence with `refinery` migrations (V0001 initial schema)
- Domain types: `Folder`, `Rule`, `Condition`, `Action`, `Schedule`,
  `Activity`, `UndoEntry`
- Rule engine: condition matcher, plan computation, action executor
  (move/delete; trash and runCommand remain pending)
- Path token resolver (date/filename/path tokens; EXIF tokens stubbed
  for v0.2.0)
- Watcher service with functional pause/resume and startup restoration
- Local-time scheduler with startup restoration and next-run reporting
- Cooperative organization cancellation and populated undo sessions
- Schedule controls for enablement, weekdays, time, and next execution
- Tray icon with Open/Pause All/Resume All/Quit menu
- IPC handlers for: app, folders, rules, files, organize, watcher,
  settings, activity, permissions
- React UI: Dashboard, Rules, History, Settings, About pages
- Zustand store (`useAppStore`) for global state
- i18n with EN + ES catalogs (ES extracted from File Arbor v1.1.2)
- CI: Rust matrix (Ubuntu/Windows/macOS), frontend checks
- Release workflow with `tauri-action`
- Apache 2.0 license, NOTICE, CREDITS, CONTRIBUTING
- `docs/SPEC.md` — full behavioral specification
- `docs/M1_ISSUES.md` — M1 milestone breakdown

### Known limitations
- `trash` and `runCommand` actions are not implemented yet
- EXIF tokens (`{camera}`, `{lens}`, etc.) resolve to empty (dropped
  from path); EXIF reading lands in v0.2.0
- Automatic updates are intentionally excluded during local testing

## [0.1.0-alpha.1] — TBD

First tagged release. See `docs/M1_ISSUES.md` for the full checklist.
