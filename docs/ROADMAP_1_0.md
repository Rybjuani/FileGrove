# FileGrove roadmap to 1.0

FileGrove is developed Linux-first. Windows remains a future compatibility
target. The repository stays private and releases remain drafts until the
release-candidate gate.

Manual testing by the product owner is deliberately concentrated near 1.0.
Earlier milestones should rely on disposable directories and automated tests;
request a small manual check only when desktop behavior cannot be verified
reliably in automation.

## 0.2 — Core file safety

- [x] Durable multi-session Undo journal
- [x] Cancellation that keeps every completed move undoable
- [x] Safe conflict handling and exact duplicate detection
- [x] Non-destructive duplicate policy by default
- [x] Managed destination folders created inside the selected root
- [x] Confirmed recursive Automatic mode with managed-folder exclusions
- [x] Manual/Automatic modes only; legacy schedules retire safely

Exit gate: all Rust and frontend tests pass with warnings denied.

## 0.3 — Automated real-filesystem coverage

- [x] Full organize/cancel/undo round-trip using disposable files
- [x] Multiple historical sessions undone newest-first
- [x] Watcher event burst, pause/resume, and restart coverage
- [x] Recursive scans with exclusions, depth, flatten, and preserve modes
- [x] Duplicate policies and every conflict behavior across real files
- [x] Crash/restart reconciliation for pending moves and Undo

Exit gate: repeatable integration suite without personal files or UI clicking.

## 0.4 — Friendly workflow and onboarding

- [x] One-click recommended categories
- [x] Visual grouped preview instead of a technical path table
- [x] Simplified custom-rule builder with friendly defaults
- [x] First-run explanation focused on safety and Automatic mode
- [ ] Compact dashboard hierarchy and clearer progress/Undo feedback
- [ ] Accessibility pass for keyboard, focus, contrast, and screen readers

Exit gate: a new user can organize a disposable folder without documentation.

## 0.5 — Linux packaging

- [x] Build `.deb` and AppImage locally
- [x] Start AppImage with an isolated clean profile
- [ ] Install, upgrade, and uninstall the `.deb` safely
- [ ] Validate desktop entry, icon, tray, autostart, and single instance
- [ ] Validate AppImage on a clean supported Linux environment
- [ ] Document supported distributions and runtime dependencies

Exit gate: packages behave like normal Linux desktop applications and never
remove user configuration during an ordinary upgrade.

## 0.9 — Release candidate

- [ ] Product-owner test using only `filegrove_tests`
- [ ] Large mixed tree test, including thousands of photos
- [ ] Stop mid-operation and undo every completed move
- [ ] Undo several previous sessions in order
- [ ] Automatic-mode soak test
- [ ] Final Spanish and English copy review
- [ ] Security, privacy, license, and contributor documentation review

Exit gate: no open issue can plausibly lose, overwrite, or silently delete a
user file.

## 1.0 — Public Linux release

- [ ] Make the repository public
- [ ] Tag the audited commit
- [ ] Publish checksums and signed release artifacts when signing is available
- [ ] Publish concise installation, recovery, and safe-use instructions
- [ ] Keep Windows work explicitly out of the Linux 1.0 release scope
