# FileGrove competitive positioning

This document records a product goal from the owner: FileGrove should make the
power normally split between File Arbor's free and paid plans available at no
cost, with a simpler experience and stronger safety guarantees.

The pricing and feature snapshot below was supplied by the product owner for
planning. It is a planning reference, not an independently verified current
price list.

## Reference competitor tiers

| Tier              | Price model                    | Main limits or capabilities                                                                                                                                                   |
| ----------------- | ------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| File Arbor Gratis | $0 forever, no card or account | One watched folder, two rules, 20 files/day, extension filter only, move to Trash, Manual mode, community support                                                             |
| File Arbor Pro    | $24.99 one-time license        | Unlimited folders and rules, regex/size/date filters, subfolder scanning, Automatic mode, scheduling, permanent deletion, custom commands, priority support, lifetime updates |

## FileGrove target

There must be no FileGrove license server, trial, paywall, account, usage cap,
HWID check, telemetry requirement, or feature split between free and paid
editions. The Linux-first product should provide the following capabilities to
every user:

| Capability                | FileGrove target                                                                                                   |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| Watched folders and rules | Unlimited, subject to local system resources and explicit safety limits                                            |
| Filters                   | Extension, filename, regex, size, date, metadata, exclusions, and friendly presets                                 |
| Subfolders                | Recursive scanning with depth, flatten/preserve, exclusions, and managed-folder loop prevention                    |
| Organization modes        | Manual Preview and Automatic watch mode, both with the same safety engine                                          |
| Recovery                  | Durable multi-session historical Undo, cancellation records, conflict stops, Activity, and clear recovery guidance |
| Duplicates and conflicts  | Keep both, rename, skip, Trash, overwrite with recovery retention, and explicit permanent deletion                 |
| Custom commands           | Available as an advanced local-user action with timeout, cancellation, fingerprint, and warning safeguards         |
| Support                   | Public documentation and community collaboration rather than paid feature gates                                    |
| Updates                   | Public releases and checksums; no paid lifetime-update entitlement                                                 |

This is a capability target, not permission to weaken safety limits. A local
file-count cap, command timeout, protected-root check, or confirmation step is a
safety boundary and does not turn into a paid plan.

## Scheduling decision

The competitor advertises scheduled executions. The current FileGrove Linux 1.0
scope intentionally retires the old scheduled/cron mode: Automatic mode is the
core workflow, and Manual mode remains available for explicit control. This is
an intentional product decision, not a missing free-versus-Pro restriction.

If the owner later decides that scheduled execution is necessary for complete
competitive parity, it must be implemented as a free, local, transparent
feature with the same Preview, pause, Activity, cancellation, and recovery
standards. It must not introduce a subscription, account, cloud service, or
silent background task.

## How FileGrove should win

Feature parity alone is not enough. FileGrove's differentiators are:

- safe defaults and disposable-folder onboarding;
- visual Preview instead of technical path lists;
- historical Undo across earlier sessions, not only the latest action;
- explicit Trash and permanent-deletion boundaries;
- automatic creation of destination folders inside the selected root;
- accessible, bilingual, calm UI for non-technical users;
- auditable local behavior with no telemetry or licensing infrastructure.

Any future roadmap item that adds a restriction, paywall, account, telemetry,
or opaque network dependency conflicts with this positioning and requires an
explicit product-owner decision.
