# Using FileGrove safely

FileGrove is designed to reduce file-organization work without asking users to
understand filesystem internals. Any organizer still changes file locations, so
begin with copies and move to personal libraries only after the release build
has passed its safety tests.

## Safest first run

1. Create a disposable folder containing copies of different file types.
2. Add only that folder to FileGrove.
3. Choose **Use recommended organization**. FileGrove creates category folders
   inside the selected root when they are needed.
4. Keep the folder in **Manual** mode.
5. Open **Preview** and review the grouped result.
6. Choose **Organize now**.
7. Confirm the files are in the expected category folders.
8. Use **Undo latest** and confirm they return to their original locations.
9. Enable **Automatic** only after the result is understood.

Do not use Downloads, Documents, Desktop, a photo library, or the only copy of
important data for a first run.

On a fresh application data directory, FileGrove creates a conservative profile
that watches **Desktop** and **Downloads** in Automatic mode and enables launch
at startup. See [`LAUNCH.md`](LAUNCH.md). Pause everything, remove those folders,
or use a separate test account before running the first test against real files.

## What the actions mean

| Action             | Recovery path                   | Recommended use                                 |
| ------------------ | ------------------------------- | ----------------------------------------------- |
| Move to folder     | FileGrove Undo                  | Normal organization                             |
| Move to Trash      | System Trash / Recycle Bin      | Advanced cleanup with OS recovery               |
| Delete permanently | None                            | Disposable files only unless absolutely certain |
| Run command        | Depends entirely on the command | Trusted advanced workflows only                 |

Only moves are restored by FileGrove Undo. Trash restoration belongs to the
operating system (Linux Trash or Windows Recycle Bin). Permanent deletion and
external command effects cannot be reversed by FileGrove.

## Conflicts and duplicates

The safe defaults are:

- **Rename** when a different file already has the destination name.
- **Keep both** when the full contents are identical.

Other options:

- **Skip** leaves the source untouched.
- **Overwrite** replaces the destination but retains a recovery copy only
  while the related Undo session remains available.
- **Send duplicate to Trash** relies on system Trash / Recycle Bin recovery.
- **Delete duplicate permanently** has no recovery path and requires explicit
  confirmation.

Do not dismiss an Undo session after overwrite until the result and retained
destination file have been verified.

## Finding duplicate files

The duplicate finder is separate from rule-based organization. It scans one
watched folder, groups files by exact size, and confirms candidates with a
streaming SHA-256 hash. The oldest copy is shown as the **Original**; the other
copies are the duplicates.

1. Open the folder card and choose **Find duplicates**.
2. Review the grouped result and the amount of wasted space.
3. Keep the original and clear any duplicate you want to retain.
4. Choose **Move selected to Trash** and confirm.
5. Restore anything you need from the system Trash / Recycle Bin.

The dialog never permanently deletes files. It sends only the selected
duplicates to the system Trash / Recycle Bin. If some files fail, the report is
rescanned so the failed files remain visible. Repository trees are skipped, and
the scan honors the folder's exclusions.

## Manual and Automatic modes

- **Manual** changes files only when **Organize now** is selected.
- **Automatic** watches the configured folder and reacts to new or changed
  files according to its assigned rules.
- Automatic can be paused and resumed at any time.
- Confirmed subfolder scanning can be used with Automatic; depth, exclusions,
  and destination layout should be reviewed first.
- Managed destination folders are excluded from reprocessing to avoid loops.
- Git, Mercurial, and SVN repository roots cannot be watched. Repository trees
  nested inside a watched folder are skipped by scanning, preview, the watcher,
  and duplicate cleanup.
- New folders scan only their root by default. Enable recursion deliberately.

For a busy folder, pause Automatic before changing many rules. Use Preview,
then resume when the configuration is ready.

## Large operations

- Keep the computer powered and the storage mounted.
- Avoid moving the same files with another application at the same time.
- Watch the progress indicator and do not assume closing the window stops the
  tray process.
- Use **Cancel** to request a safe stop. Completed moves remain recorded for
  Undo.
- If an operation reports any failure, review Activity before retrying.
- Repeatedly use **Undo latest** to restore older sessions newest-first.

## Rules that need extra care

- A broad condition such as a common extension can match thousands of files.
- Rule priority matters: the first enabled matching rule wins.
- A destination outside the selected root is an advanced configuration.
- Regular expressions should be tested against disposable names.
- Command rules run with the permissions of the signed-in user.
- A command can access the network or alter files outside FileGrove's safety
  boundary. FileGrove cannot undo those external effects.

Use the command test button with a disposable sample file before enabling the
rule, and keep automatic reruns disabled unless repeated execution is intended.

## When to stop

Pause FileGrove and preserve the current state if:

- Preview and observed results disagree.
- A file appears missing from both its source and intended destination.
- Undo reports a partial failure or an occupied original location.
- An interrupted delete or command is marked for review.
- A mounted drive disconnects during organization.

Do not dismiss the related Undo/recovery record. Follow `RECOVERY.md` and retain
the test folder for diagnosis.
