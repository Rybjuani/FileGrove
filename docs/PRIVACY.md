# FileGrove privacy notice

## Summary

FileGrove organizes files locally. The current application has no account,
license server, telemetry, analytics, advertising, cloud sync, or automatic
update client.

## Data processed on the computer

To provide its features, FileGrove may read and store:

- Paths and names of folders selected by the user.
- Rules, settings, exclusions, and destination templates.
- File names, paths, sizes, dates, and matching metadata.
- Local image EXIF metadata needed by a configured destination token.
- Activity results, errors, destinations, and command output.
- Undo journal paths and retained overwrite-recovery references.
- Command fingerprints used to prevent unintended reruns.

This application state is stored in a local SQLite database under the per-user
application data directory: `%APPDATA%\app.filegrove.desktop` on Windows and the
XDG data directory on Linux. See `RECOVERY.md` for the exact paths and safe
backup procedure.

## Data not sent by FileGrove

The current application does not send FileGrove usage, filenames, paths, EXIF
metadata, Activity, or identifiers to a FileGrove service. There is no FileGrove
service endpoint in the application.

Desktop notifications are handled locally by the operating system.

## Important exceptions

- A user-authored command rule can run any trusted shell command with the
  current user's permissions. That command may transmit data or contact a
  network service. Its behavior is controlled by the command, not FileGrove.
- Installing packages or visiting project links uses external tools outside the
  FileGrove application.
- The Windows installer may download the Microsoft WebView2 bootstrapper when
  the runtime is missing. That request is made by the installer, not by the
  running application.
- The operating system, desktop, filesystem, security software, or crash
  reporter may keep its own records under separate policies.

## Retention and deletion

- Activity remains until the user clears it.
- Move Undo sessions remain until used or explicitly dismissed.
- Folder and rule configuration remains until changed or removed.
- Command fingerprints remain to prevent repeated command execution.
- Uninstalling the executable leaves per-user application data in place. The
  Windows uninstaller asks before deleting it; Linux package validation is still
  pending.

Deleting the application data directory permanently removes settings, Activity,
and FileGrove recovery records, including historical Undo. Quit FileGrove and
make a backup before considering that action. Removing FileGrove records does
not reverse file moves, restore Trash, or undo command effects.

## Diagnostic information

The current build writes diagnostic logs to terminal output and does not create
a persistent FileGrove log file by default. If diagnostic output is shared for
support, redact personal paths, filenames, command text, and command output.

Never provide personal files as part of a bug report unless their contents were
created specifically for public reproduction.

## Future changes

Telemetry, accounts, licensing, and paywalls are explicitly out of scope.
Automatic updates are excluded from the current local-testing phase. Any future
feature that changes network or data-processing behavior requires an explicit
product decision and an updated privacy notice before release.
