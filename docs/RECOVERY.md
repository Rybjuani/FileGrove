# File recovery and Undo

This guide explains FileGrove's recovery boundaries. It does not replace a
separate backup of important personal files.

## Undo moved files

Each completed organization batch can create a durable Undo session. The folder
card shows how many sessions remain and how many files belong to the newest one.

1. Stop or pause further organization of that folder.
2. Select **Undo latest**.
3. Wait for the result message.
4. Verify the files at their original locations.
5. Repeat **Undo latest** to go further back in history.

Sessions must be undone newest-first. This prevents an older rollback from
colliding with changes made by a newer batch.

## Safety stops during Undo

Undo refuses to overwrite an unrelated file. If another file now occupies the
original location, FileGrove leaves both files unchanged and reports the
conflict.

If only part of a batch can be restored:

- Do not dismiss the Undo record.
- Review Activity and the reported source and destination locations.
- Move unrelated blocking files only after identifying them.
- Retry Undo; successfully restored entries are not repeated.
- Preserve both copies whenever the final state is ambiguous.

An interrupted move can be reconciled at the next launch. Ambiguous states are
marked for review instead of being guessed or silently repeated.

## Overwritten destinations

With **Overwrite**, FileGrove preserves the prior destination content as part
of the related Undo session. Undo restores both the moved source and the prior
destination when it can do so safely.

That recovery copy is retained only while the session remains available. Do not
dismiss the session until the overwrite result is accepted.

## Trash and permanent deletion

- **Move to Trash** has no FileGrove Undo. Restore the item through the Linux
  Trash or the Windows Recycle Bin.
- On Windows the Recycle Bin is per volume: an item removed from `F:` is kept
  in `F:\$RECYCLE.BIN`, not on the Windows drive. FileGrove never relocates or
  redirects the system Trash / Recycle Bin.
- **Delete permanently** has no recovery mechanism in FileGrove or the system
  Trash.
- Exact-duplicate permanent deletion has the same limitation.
- The visual duplicate finder never permanently deletes. It sends only the
  selected duplicates to the system Trash / Recycle Bin.

If a permanent deletion was interrupted, Activity records that its result needs
review. Inspect the original path before authorizing any retry.

## Duplicate cleanup

The duplicate finder groups files by exact size and confirms them with a
streaming SHA-256 hash. The oldest copy is marked as the original; only the
other copies can be selected. The operation sends the selected duplicates to
the system Trash / Recycle Bin.

- Restore a duplicate from the Windows Recycle Bin or the Linux Trash.
- If the operation reports a partial failure, the dialog rescans the folder so
  the files that were not moved remain visible.
- FileGrove does not keep a separate duplicate-cleanup journal; the system
  Trash / Recycle Bin is the recovery path.

## Interrupted commands

FileGrove keeps a fingerprint ledger to avoid automatically repeating a command
after a crash or uncertain shutdown. An interrupted command remains blocked
until the user reviews its external effect and explicitly permits one retry.

FileGrove cannot undo changes made by a shell command, another application, a
remote service, or a script.

## Back up FileGrove state

On Windows, the application data directory is:

```text
%APPDATA%\app.filegrove.desktop\
```

On Linux, when `XDG_DATA_HOME` is set, it is:

```text
$XDG_DATA_HOME/app.filegrove.desktop/
```

Otherwise, the default directory is
`~/.local/share/app.filegrove.desktop/`.

It contains `app.db` and may temporarily contain SQLite `-wal` and `-shm`
files. The database stores settings, watched folders, rules, Activity, Undo
journals, and command fingerprints.

For a consistent backup:

1. Quit FileGrove from the tray menu; closing the window may only hide it.
2. Confirm no `filegrove` process remains.
3. Copy the entire `app.filegrove.desktop` directory to a safe location.
4. Keep the backup together with the FileGrove version that created it.

Do not edit the SQLite database by hand. Do not restore only `app.db` while
FileGrove is running.

## Restoring application state

Restore application data only when FileGrove is fully stopped and after keeping
a copy of the current directory. Prefer the same application version first; let
normal migrations run before upgrading further.

Restoring the database restores FileGrove's records, not files that were
permanently deleted, sent to the system Trash, or changed by external commands.
Undo can succeed only while the filesystem paths and retained recovery copies
still exist.

## Information to preserve for a bug report

- FileGrove version and commit if known.
- Windows version and build number, or Linux distribution, desktop, and
  X11/Wayland session.
- The exact action and whether Manual or Automatic was active.
- The visible error and relevant Activity entry.
- A directory listing of the disposable source and destination.
- Terminal diagnostic output when available.

Redact personal paths and filenames. Never upload private files merely to prove
a recovery problem.
