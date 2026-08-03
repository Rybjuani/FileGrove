# FileGrove security policy

## Release status

FileGrove is currently a private Linux release candidate. It should be used
only with disposable copies until the package and release-candidate gates in
`ROADMAP_1_0.md` pass.

## Reporting a vulnerability

Do not publish a file-loss, command-execution, path-traversal, or privacy issue
with personal paths or files attached.

While the repository is private, report it directly to the repository owner or
through the private collaboration channel that granted access. Before making
the repository public, enable GitHub private vulnerability reporting and update
this section with the final supported contact route.

Include:

- FileGrove version or commit.
- Linux distribution and desktop session.
- Minimal reproduction using disposable files.
- Expected and observed filesystem state.
- Whether Automatic, recursion, overwrite, delete, or a command rule was active.
- Redacted Activity details and terminal output.

There is no guaranteed response SLA during private release preparation. A
plausible data-loss issue blocks release until resolved and retested.

## Security boundaries

FileGrove runs with the current user's permissions. It must never require or
request root privileges for normal use.

The backend provides these safeguards:

- Canonical path validation rejects parent traversal and protected system roots.
- Watched-folder overlap checks prevent ambiguous nested ownership.
- Permanent deletion is restricted to the watched root and explicit warning path.
- Move operations journal recovery state before changing the filesystem.
- Existing destinations are never silently overwritten by Undo.
- Overwrite retains the old destination while its Undo session exists.
- Exact duplicates are compared byte-for-byte before duplicate policy applies.
- Recursive scans enforce exclusions, depth limits, and a 100,000-file cap.
- Automatic events are coalesced and recover through a catch-up scan after bursts.
- Commands have timeouts, output limits, process-tree cancellation, and durable
  fingerprints that prevent uncertain automatic reruns.
- Interrupted moves, deletes, and commands are reconciled or marked for review
  instead of being silently repeated.

## Actions outside the recovery boundary

FileGrove cannot undo:

- Permanent deletion.
- Restoration from the Linux desktop Trash.
- Effects of user-authored commands or scripts.
- Changes made concurrently by another program, sync service, or user.
- Hardware failure, filesystem corruption, or removal of a mounted drive.

The command action intentionally invokes the platform shell. A trusted command
can read or modify anything accessible to the current user and may use the
network. Treat command rules as code, not as ordinary file-matching rules.

## Network and update posture

The current application contains no account, license, telemetry, analytics, or
automatic-updater client. File organization is local. User-authored commands
remain outside this guarantee and may create their own network traffic.

Release artifacts are not trusted merely because they have a FileGrove name.
Verify published checksums and signatures when signing becomes available.

## Dependency and release review

Before Linux 1.0:

- Review locked Rust and npm dependency advisories.
- Generate a complete third-party license inventory from the exact lockfiles.
- Normalize stale beta-looking Tauri manifest ranges and rebuild packages.
- Run CI and package tests on the exact tagged commit.
- Publish checksums and the supported Linux matrix.
