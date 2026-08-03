# Installing FileGrove on Linux

FileGrove is still a private release candidate. The packages described here
exist locally, but the full clean-machine installation matrix has not passed
yet. Use only disposable folders until the 1.0 release gate is complete.

## Available package formats

The current `amd64` build produces:

- `FileGrove_<version>_amd64.deb`
- `FileGrove_<version>_amd64.AppImage`

Published releases must provide SHA-256 checksums. Verify the checksum before
running or installing a downloaded artifact:

```bash
sha256sum FileGrove_<version>_amd64.deb
sha256sum FileGrove_<version>_amd64.AppImage
```

Compare the complete result with the checksum published for that exact release.
The current private artifacts are not signed distribution releases.

## Debian package

The generated package is named `file-grove`. It installs:

- `/usr/bin/filegrove`
- `/usr/share/applications/FileGrove.desktop`
- FileGrove icons under `/usr/share/icons/hicolor/`

Install from the directory containing the artifact:

```bash
sudo apt install ./FileGrove_<version>_amd64.deb
```

Using `apt` is preferred over invoking `dpkg` directly because `apt` resolves
the package's runtime dependencies. The current package declares:

- `libgtk-3-0`
- `libwebkit2gtk-4.1-0`
- `libayatana-appindicator3-1`

After installation, start **FileGrove** from the desktop application menu or
run `filegrove` from a terminal. Terminal launch is useful during testing
because the current build writes diagnostic logs to standard error instead of a
persistent application log file.

### Upgrade

Install the newer `.deb` with the same `apt install ./...` command. Before an
upgrade test, quit FileGrove from its tray menu and back up its application data
directory as described in `RECOVERY.md`.

An ordinary package upgrade must preserve settings, rules, Activity, command
fingerprints, and Undo journals. This remains a release-candidate test; it must
be verified using two distinct package versions.

### Uninstall

Remove the installed package with:

```bash
sudo apt remove file-grove
```

Package removal should not erase per-user FileGrove data. That behavior still
requires installation testing. Do not manually delete the application data
directory: it contains configuration, Activity, and the records required by
historical Undo.

## AppImage

The AppImage does not require system-wide installation:

```bash
chmod +x FileGrove_<version>_amd64.AppImage
./FileGrove_<version>_amd64.AppImage
```

It still uses the normal per-user FileGrove data directory; it is not a
self-contained or disposable data profile. Desktop-menu integration is not
automatically guaranteed for an AppImage.

The AppImage has started successfully on this development computer with an
isolated clean profile. Validation on a separate clean supported Linux
environment remains pending.

## Application data

When `XDG_DATA_HOME` is set, FileGrove stores its SQLite database at:

```text
$XDG_DATA_HOME/app.filegrove.desktop/app.db
```

Otherwise, the default location is
`~/.local/share/app.filegrove.desktop/app.db`.

SQLite may also create `app.db-wal` and `app.db-shm` beside it while FileGrove
is running. Quit the application completely before backing up or restoring the
directory.

Uninstalling the executable and deleting application data are deliberately
separate actions. See `RECOVERY.md` before touching this directory.

## Current compatibility evidence

| Environment                                          | Status                             |
| ---------------------------------------------------- | ---------------------------------- |
| Linux Mint 22.3, XFCE, X11, amd64                    | Development and local bundle build |
| AppImage with isolated profile on the development PC | Starts successfully                |
| `.deb` install/upgrade/uninstall                     | Not yet validated                  |
| Separate clean Linux machine or VM                   | Not yet validated                  |
| Wayland desktop session                              | Not yet validated                  |
| ARM64                                                | No artifact currently produced     |

This table is evidence, not a broad support promise. The supported-distribution
matrix will be declared only after package testing.

## Troubleshooting during release testing

- Start from a terminal and save its output if the window, tray, or watcher
  fails.
- Confirm the watched folder still exists and is writable by the current user.
- Do not run FileGrove as root. Files and commands should use the permissions of
  the signed-in user.
- Do not retry a failed destructive action until Activity and both filesystem
  locations have been reviewed.
- Report the FileGrove version, distribution, desktop, X11/Wayland session, and
  exact reproduction steps. Redact personal paths and filenames.
