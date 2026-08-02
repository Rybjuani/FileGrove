# Local validation checklist

FileGrove 0.1.0 is being tested only on the development PC. This file replaces
the original scaffold milestone: the rule engine, watcher, scheduler, undo,
EXIF support, command actions, tray integration, and advanced UI are now
implemented.

No GitHub project, public release, signed updater, or updater service should be
created during this phase.

## Automated checks

- [x] TypeScript type checking
- [x] ESLint with zero warnings
- [x] Prettier formatting check
- [x] Frontend unit tests
- [x] Frontend production build
- [x] Rust formatting
- [x] Rust Clippy with warnings denied
- [x] Rust tests for rules, scanning, paths, EXIF, commands, and storage
- [ ] Production desktop bundle build on the development PC

## Disposable-folder smoke test

Use newly created folders containing copies of test files. Do not use the real
Downloads, Documents, Desktop, or photo library for the first runs.

- [ ] Add, rename, and remove a watched folder
- [ ] Confirm overlapping watched folders are rejected
- [ ] Create rules for move, Recycle Bin, permanent delete, and command actions
- [ ] Preview each rule before organizing
- [ ] Exercise rename, skip, and overwrite conflicts
- [ ] Exercise recursive depth, exclusions, preserve structure, and flatten
- [ ] Confirm automatic mode reacts only to the changed files
- [ ] Confirm scheduled mode runs at the selected local time
- [ ] Cancel an active organization and retry it
- [ ] Undo a move and confirm existing destinations are never overwritten
- [ ] Confirm Recycle Bin actions are recoverable through Windows
- [ ] Confirm permanent delete always requires the explicit warning path
- [ ] Confirm command test output, timeout, cancellation, and no accidental rerun
- [ ] Confirm EXIF photo tokens and fallback file dates
- [ ] Restart the app and confirm folders, rules, settings, and schedules persist
- [ ] Confirm tray pause/resume/open/quit and single-instance behavior
- [ ] Review activity history and background error notifications

## Installer test (later)

After sustained disposable-folder testing:

- [ ] Install the locally built package
- [ ] Launch, quit, reinstall, and uninstall cleanly
- [ ] Confirm application data survives an ordinary app restart
- [ ] Confirm no updater or external network dependency is present

## Deferred intentionally

- GitHub publication and CI workflows
- Public release artifacts and code signing
- Signed automatic updates
- Cross-platform installer validation outside this Windows PC
