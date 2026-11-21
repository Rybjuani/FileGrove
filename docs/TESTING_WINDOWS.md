# FileGrove 0.1.0 — Windows testing guide

FileGrove `0.1.0` is a **private local test build**, not a signed public release.
Use disposable copies, a dedicated test folder, or a separate Windows account/VM
until the release gates in [`ROADMAP_1_0.md`](ROADMAP_1_0.md) pass. Do not test
the first build against the only copy of important files.

## Artifact

Installer:

```text
src-tauri/target/release/bundle/nsis/FileGrove_0.1.0_x64-setup.exe
```

- NSIS, x64, per-user installation (`currentUser`); Windows does not request
  administrator rights.
- Default install directory: `%LOCALAPPDATA%\FileGrove`.
- WebView2 mode: Tauri `downloadBootstrapper`. Windows 10/11 normally includes
  the WebView2 Runtime; if it is missing, the installer downloads it and needs
  internet access.
- The installer is **unsigned**, so Windows SmartScreen may show a warning.
- Size: 2,541,285 bytes (about 2.42 MiB).
- SHA-256:
  `db606b64c4cf01ce09a4d38a01b0a5588d75253019bb7c52c253e7b3c1fdc493`

Verify the file before running it:

```powershell
Get-FileHash .\FileGrove_0.1.0_x64-setup.exe -Algorithm SHA256
```

- The portable release binary is
  `src-tauri/target/release/filegrove.exe` (SHA-256
  `caca8ce123cc5db23260be73e20c867c42efbbcdf870e60d9f2b80c5c6e7b419`). It runs
  without installation but uses the same per-user application data directory.

## Read this before the first launch

On a fresh application data directory, FileGrove does not add folders,
assign rules, organize files in the background, or enable launch at startup.
The onboarding dialog remains available. Add a disposable folder yourself;
new folders start in Manual mode and stopped. Enable Automatic mode and launch
at startup only if you choose to test those settings.

To stop FileGrove completely, choose **Quit** in the tray; closing the window
may only hide it. Existing saved configurations are preserved when updating.

## Test data and recovery

- Keep important files out of every watched root and destination.
- Use copies in a folder such as `C:\FileGroveTests`.
- Keep an untouched backup outside every watched root.
- Moves are restored with **Undo latest** on the folder card.
- Duplicate cleanup and Trash actions are restored from the Windows Recycle Bin.
  The Recycle Bin is per volume, so files removed from a secondary drive stay on
  that drive (for example, `F:\$RECYCLE.BIN`).
- Permanent deletion and command actions have no FileGrove recovery.
- Application state lives at
  `%APPDATA%\app.filegrove.desktop\app.db` (plus temporary `-wal` and `-shm`
  files). Quit FileGrove from the tray before copying it.
- The uninstaller asks whether to delete application data. Leave that option
  unchecked to preserve settings, rules, Activity, and Undo history.

## Suggested test sequence

### 1. Install and desktop shell

- [ ] Install without an administrator prompt and confirm the Start menu entry.
- [ ] Launch FileGrove and confirm the main window opens.
- [ ] Launch it a second time and confirm the existing window is focused.
- [ ] Open the tray menu and exercise **Open**, **Pause all**, **Resume all**,
      and **Quit**.
- [ ] Disable launch at startup and confirm the entry disappears from Task
      Manager → Startup; re-enable it and confirm it returns.
- [ ] Close the window and verify the tray process remains available; then quit
      from the tray and confirm the process exits.

### 2. Folders, rules, preview, and manual organization

- [ ] Add a disposable folder and confirm a Git, Mercurial, or SVN repository
      root is rejected with a clear message.
- [ ] On Windows, select a secondary drive root (for example `D:\`) and confirm
      it is accepted; confirm the system drive root (normally `C:\`) is rejected
      as protected.
- [ ] With a secondary drive watched recursively, confirm `$RECYCLE.BIN`,
      `System Volume Information`, `Recovery`, `Config.Msi`, `.Trash-1000`, and
      `SteamLibrary` are skipped.
- [ ] Add a parent folder that contains a repository and confirm files outside
      the repository are visible while repository contents are ignored.
- [ ] Apply the recommended categories and review the grouped **Preview**.
- [ ] Run **Organize now** in Manual mode and verify each file reaches its
      destination.
- [ ] Run **Undo latest** and verify the files return to their original paths.
- [ ] Create a name conflict and exercise rename, skip, and overwrite behavior
      using copies only.
- [ ] Confirm a file already at its destination is skipped rather than moved
      again, and confirm a managed destination folder is not reprocessed.

### 3. Automatic mode and restart

- [ ] Enable Automatic mode for a disposable folder and add files in small
      bursts.
- [ ] Pause Automatic, add more files, resume, and confirm the catch-up
      behavior is safe.
- [ ] Restart FileGrove with Automatic folders active and confirm watching is
      restored.
- [ ] Leave Automatic mode running for a soak period and verify it does not
      loop, repeatedly move the same files, or process repository contents.
- [ ] Review **Activity** for every background error; no failure should be
      silent.

### 4. Duplicate clean-up

- [ ] Create three files with identical contents and different names, plus one
      unique file, inside a disposable folder.
- [ ] Choose **Find duplicates** on the folder card and wait for the scan.
- [ ] Confirm the oldest copy is marked **Original** and the other copies are
      selected as duplicates.
- [ ] Deselect one duplicate, move the selection to the Recycle Bin, and verify
      the confirmation and the remaining report.
- [ ] Restore the moved files from the Windows Recycle Bin and run the scan
      again.
- [ ] Scan a folder containing a repository and confirm repository contents are
      not reported as duplicates.
- [ ] If a Trash operation fails, confirm the report refreshes and the failed
      files remain visible instead of disappearing.
- [ ] Switch between English and Spanish and confirm the dialog, accessibility
      labels, counts, and buttons are translated.

### 5. Optional advanced checks

- [ ] Exercise EXIF destinations with copied photos.
- [ ] Test a harmless command rule, including timeout, cancellation, output
      capture, and restart protection.
- [ ] Cancel a large Manual organization and undo every completed move.
- [ ] Run several batches and undo them newest-first.
- [ ] Confirm Undo never overwrites a file newly placed at an original location.

## Known limitations in this test build

- The installer is unsigned and there is no automatic updater.
- The duplicate scan runs without a progress bar or cancel button; very large
  folders can take a while and the dialog may appear busy.
- The Playwright end-to-end project is not implemented yet; the automated
  coverage is the Rust suite plus the Vitest suite.
- The black-box `watcher_service` integration binary fails to load on this
  development PC with `STATUS_ENTRYPOINT_NOT_FOUND` before executing any test
  code. The same suite runs in Linux CI. This is treated as an environment
  issue until it is reproduced on a second Windows machine; report any actual
  watcher failure separately.
- Linux clean-machine installation, upgrade, and AppImage validation remain
  open release gates. The current multi-day test target is Windows.

## Reporting a problem

Include:

- FileGrove version (`0.1.0`) and the approximate time.
- Windows version and build number (`winver`).
- Whether Manual or Automatic mode was active, and whether recursion, a
  command, overwrite, permanent deletion, or duplicate cleanup was involved.
- Exact reproduction steps using disposable files.
- Expected and observed filesystem state, including both paths.
- The relevant **Activity** entry and any visible error.
- For development-only diagnostics, run `npm run tauri:dev` from a terminal with
  `RUST_LOG=debug` and include the relevant output.

Redact personal paths and filenames. Never upload private files just to prove a
recovery problem. If a file appears to be missing, stop the test, preserve the
folder and application data, and do not repeat the destructive action.
