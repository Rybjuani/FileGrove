# M1 — Skeleton & Scaffolding

> **Milestone goal**: Empty but bootable Tauri + React app with CI,
> navigation, SQLite storage, and i18n (EN+ES) wired up.
>
> **Estimated effort**: 1 week (1 dev, half-time).
>
> **Definition of done**: `npm run tauri dev` opens a window showing the
> Dashboard with sidebar navigation. `cargo test` and `npm run test`
> both pass. CI is green on Windows, macOS, Linux.

---

## Issues

Each issue below is ready to be copy-pasted into GitHub. Labels:
`milestone:M1`, `type:infra`, `type:feat`, `type:test`, `type:docs`.

---

### Issue 1 — Verify scaffold compiles and runs

**Labels**: `milestone:M1`, `type:infra`, `priority:P0`

**Description**:

The scaffold was generated offline. Before going further, verify it
boots end-to-end on a fresh clone.

**Acceptance criteria**:

- [ ] `npm install` succeeds without errors
- [ ] `npm run typecheck` passes
- [ ] `npm run lint` passes with zero warnings
- [ ] `cd src-tauri && cargo check` succeeds on Linux
- [ ] `cd src-tauri && cargo test` passes (rule matcher, schedule cron builder, token resolver)
- [ ] App icons are generated (see `src-tauri/icons/README.md`)
- [ ] `npm run tauri dev` opens a window titled "FileGrove"
- [ ] Window shows the sidebar with all 5 nav items
- [ ] Clicking each nav item switches the page
- [ ] Settings page can toggle "minimize to tray" and the value persists across restarts

**Test plan**:

1. Clone repo, run install commands above.
2. Open DevTools (F12) — verify no console errors.
3. Toggle theme dark ↔ light — UI updates immediately.
4. Switch language EN ↔ ES — all visible strings translate.
5. Restart the app — settings persist.

---

### Issue 2 — Add app icons and tray icon

**Labels**: `milestone:M1`, `type:infra`, `priority:P0`

**Description**:

The scaffold ships without icons. Generate them from a 1024×1024 source
PNG using `tauri icon`.

**Acceptance criteria**:

- [ ] Source PNG (`design/icon-source.png`) committed
- [ ] `npm run tauri icon design/icon-source.png` produces all required formats in `src-tauri/icons/`
- [ ] App icon visible in:
  - [ ] Window title bar (Windows, Linux)
  - [ ] Dock (macOS)
  - [ ] System tray (all platforms)
  - [ ] Task manager / activity monitor
- [ ] Tray icon shows context menu: Open, Pause All, Resume All, Quit
- [ ] Clicking "Open" focuses the main window
- [ ] Clicking "Quit" exits the app cleanly

**Notes**:

- Source PNG should be a flat dark slate (#0f172a) background with an
  emerald (#10b981) "trees" / "grove" motif. Use Lucide's `trees` icon
  as inspiration.
- macOS icon should follow the Big Sur squircle convention.

---

### Issue 3 — SQLite migration runner

**Labels**: `milestone:M1`, `type:feat`, `priority:P0`

**Description**:

The scaffold ships with `V0001__initial.sql` but the migration runner
needs to be verified end-to-end: schema creation, idempotent re-runs,
and forward-only guarantees.

**Acceptance criteria**:

- [ ] On first launch, `<app_data_dir>/app.db` is created
- [ ] All 5 tables exist: `settings`, `folders`, `rules`, `activity`, `undo_entries`
- [ ] Default settings rows are inserted
- [ ] On second launch, no migration runs (refinery tracks version)
- [ ] Adding a `V0002__test.sql` and restarting applies it
- [ ] Removing `V0002__test.sql` and restarting does **not** roll back
- [ ] Database file is WAL mode (`PRAGMA journal_mode` returns `wal`)

**Test plan**:

```rust
#[test]
fn migrations_are_idempotent() {
    let db = Database::open_in_memory().unwrap();
    db.run_migrations().unwrap();
    db.run_migrations().unwrap(); // second run is a no-op
    let conn = db.conn.lock();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM settings WHERE key = 'theme'", [], |r| r.get(0))
        .unwrap();
    assert!(count >= 1);
}
```

---

### Issue 4 — Settings store round-trip

**Labels**: `milestone:M1`, `type:feat`, `priority:P0`

**Description**:

The IPC `settings:get` / `settings:set` handlers are stubbed. Verify
that every setting on the Settings page can be saved and reloaded.

**Acceptance criteria**:

- [ ] All settings from `docs/SPEC.md` §4.2 are present in the DB after first launch
- [ ] Settings page UI binds to all of them
- [ ] Changing theme updates the UI live
- [ ] Changing language updates i18next live
- [ ] All toggles persist across restarts
- [ ] Conflict behavior select persists across restarts

**Test plan**:

1. Open Settings page.
2. Toggle every setting; change every dropdown.
3. Quit and relaunch the app.
4. Verify every change persisted.

---

### Issue 5 — Folder CRUD smoke test

**Labels**: `milestone:M1`, `type:feat`, `priority:P0`

**Description**:

The folder IPC handlers (`folders:add`, `folders:list`,
`folders:remove`, `folders:updateName`, `folders:updateMode`) are
implemented but untested through the UI.

**Acceptance criteria**:

- [ ] Clicking "Add folder" on Dashboard opens OS folder picker
- [ ] Selecting a folder adds it to the grid
- [ ] Renaming a folder updates the card title
- [ ] Changing mode (single/auto/scheduled) updates the card badge
- [ ] Removing a folder removes the card
- [ ] Restarting the app shows all folders again (persistence)
- [ ] Adding the same folder twice is a no-op (returns existing)

**Test plan**:

1. Add 3 folders: `~/Downloads`, `~/Documents`, `~/Pictures`.
2. Rename each.
3. Cycle each through all 3 modes.
4. Remove one.
5. Restart — verify state.

---

### Issue 6 — Rule CRUD smoke test

**Labels**: `milestone:M1`, `type:feat`, `priority:P1`

**Description**:

Same as Issue 5 but for rules. The rule editor in the scaffold is
minimal (extension + destination only); this issue verifies that the
basic happy path works.

**Acceptance criteria**:

- [ ] Clicking "New rule" opens the editor modal
- [ ] Filling name + extensions + destination and saving creates the rule
- [ ] Rule appears in the list
- [ ] Toggling enabled/disabled persists
- [ ] Deleting a rule removes it from the list
- [ ] Restarting shows all rules again

**Test plan**:

1. Create 3 rules: Photos (.jpg,.png → Photos), Docs (.pdf → Docs),
   Videos (.mp4 → Videos).
2. Toggle each off and back on.
3. Delete one.
4. Restart — verify.

---

### Issue 7 — i18n: complete EN catalog

**Labels**: `milestone:M1`, `type:feat`, `priority:P1`

**Description**:

The `en.json` catalog was hand-written to match the scaffold's UI. As
pages get filled out, missing keys will surface. This issue tracks
keeping EN complete.

**Acceptance criteria**:

- [ ] Every `t('...')` call in the codebase resolves to a non-empty string
- [ ] No `[missing: ...]` warnings in the console during normal use
- [ ] `npm run typecheck` passes (an i18next type checker plugin could
      enforce this at build time — optional)

**How to find missing keys**:

```bash
# Run the dev server, browse every page, watch the console for
# i18next warnings. Or grep:
grep -roE "t\\(['\"][^'\"]+['\"]" src/ | sort -u
```

---

### Issue 8 — i18n: verify ES catalog parity

**Labels**: `milestone:M1`, `type:feat`, `priority:P1`

**Description**:

The `es.json` catalog was extracted from the original File Arbor v1.1.2.
It's ~26 namespaces. Verify parity with EN — every EN key should exist
in ES, and vice versa.

**Acceptance criteria**:

- [ ] Run a parity script: every key in EN exists in ES
- [ ] No extra ES keys that aren't in EN (would be dead strings)
- [ ] Switching language to ES shows fully-translated UI
- [ ] All ES strings use proper UTF-8 (no `Ã©` etc.)

**Parity script**:

```bash
node -e "
const en = require('./src/i18n/locales/en.json');
const es = require('./src/i18n/locales/es.json');
const flatten = (o, p='') => Object.entries(o).flatMap(([k,v]) => typeof v === 'object' ? flatten(v, p+k+'.') : [p+k]);
const ek = new Set(flatten(en));
const sk = new Set(flatten(es));
console.log('Missing in ES:', [...ek].filter(k => !sk.has(k)));
console.log('Missing in EN:', [...sk].filter(k => !ek.has(k)));
"
```

---

### Issue 9 — CI: Rust matrix builds green

**Labels**: `milestone:M1`, `type:infra`, `priority:P0`

**Description**:

The CI workflow in `.github/workflows/ci.yml` runs Rust fmt/clippy/test
on Ubuntu, Windows, macOS. Make sure all three pass.

**Acceptance criteria**:

- [ ] Push to `main` triggers CI
- [ ] All 3 OS jobs pass
- [ ] `cargo fmt --check` passes (run `cargo fmt` once to format)
- [ ] `cargo clippy -- -D warnings` passes (fix all warnings)
- [ ] `cargo test` passes on all 3 OS

**Common fixes**:

- Linux: install `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf`
- Windows: nothing extra needed
- macOS: nothing extra needed

---

### Issue 10 — CI: Frontend checks green

**Labels**: `milestone:M1`, `type:infra`, `priority:P0`

**Description**:

Frontend CI runs typecheck, lint, format check, and unit tests.

**Acceptance criteria**:

- [ ] `npm run typecheck` passes
- [ ] `npm run lint` passes with 0 warnings
- [ ] `npm run format:check` passes
- [ ] `npm run test -- --run` passes
- [ ] Vitest coverage ≥ 60% on `src/lib/` and `src/stores/`

---

### Issue 11 — Docs: architecture overview

**Labels**: `milestone:M1`, `type:docs`, `priority:P2`

**Description**:

`docs/SPEC.md` is the spec; `docs/ARCHITECTURE.md` should be the
narrative companion: how the modules actually fit together, where to
find things, common patterns.

**Acceptance criteria**:

- [ ] `docs/ARCHITECTURE.md` written
- [ ] Covers: Tauri setup, IPC flow, SQLite schema, watcher lifecycle,
      scheduler lifecycle, tray lifecycle, theme system, i18n system
- [ ] Includes a "Where to look for X" cheat sheet
- [ ] Linked from `README.md`

---

### Issue 12 — Docs: IPC reference auto-generated

**Labels**: `milestone:M1`, `type:docs`, `priority:P2`

**Description**:

`docs/IPC.md` should be the canonical reference for every IPC channel.
Ideally auto-generated from the Rust signatures so it can't drift.

**Acceptance criteria**:

- [ ] Every `#[tauri::command]` fn is documented
- [ ] Each entry: signature, args, return type, error codes, notes
- [ ] Either a script regenerates it, or there's a CI check that
      warns when a new command isn't documented
- [ ] Renderer `ipc` namespace in `src/ipc/index.ts` matches exactly

---

### Issue 13 — Set up GitHub Projects board

**Labels**: `milestone:M1`, `type:infra`, `priority:P2`

**Description**:

Create the project board so issues can be triaged visually.

**Acceptance criteria**:

- [ ] Project created: "FileGrove Roadmap"
- [ ] Views: "Roadmap" (by milestone), "Board" (Kanban by status)
- [ ] All M1 issues (this file) imported
- [ ] Labels created: `type:feat`, `type:fix`, `type:docs`,
      `type:infra`, `type:test`, `priority:P0/P1/P2`, `milestone:M1`
      through `milestone:M16`
- [ ] Branch protection on `main`: require PR review, require CI green

---

### Issue 14 — Smoke E2E test

**Labels**: `milestone:M1`, `type:test`, `priority:P1`

**Description**:

A single Playwright test that boots the app and exercises the happy
path. This catches regressions in the bootstrap path early.

**Acceptance criteria**:

- [ ] `tests/e2e/smoke.spec.ts` exists
- [ ] Test: app launches, sidebar visible, click through every page
- [ ] Test: add a folder, verify it appears, remove it, verify it's gone
- [ ] Test: change theme, verify class toggles on `<html>`
- [ ] Test: change language to ES, verify "Panel" appears in sidebar
- [ ] `npm run e2e` runs in CI on Linux

---

### Issue 15 — Tag v0.1.0-alpha.1

**Labels**: `milestone:M1`, `type:release`, `priority:P0`

**Description**:

First tagged release. Not for end users — just a checkpoint.

**Acceptance criteria**:

- [ ] All P0 issues closed
- [ ] `CHANGELOG.md` initialized with v0.1.0-alpha.1 entry
- [ ] Version bumped in `package.json`, `src-tauri/Cargo.toml`,
      `src-tauri/tauri.conf.json`
- [ ] Git tag `v0.1.0-alpha.1` pushed
- [ ] Release workflow triggered, produces unsigned installers for
      Win/macOS/Linux
- [ ] Smoke-test each installer in a VM
- [ ] GitHub Release notes summarize what's in and what's not

**What's in**:

- Bootable app, navigation, settings, folder CRUD, rule CRUD, i18n
  (EN+ES), tray, SQLite persistence, CI

**What's NOT in (deferred to M2+)**:

- Rule engine execution (rules exist but don't run)
- Watcher (auto mode doesn't trigger)
- Scheduler (scheduled mode doesn't fire)
- Undo, activity history (UI exists, no data)
- EXIF, run-command (postponed to v0.2.0)

---

## Out of scope for M1

Anything that requires the rule engine to actually execute (M3), the
watcher to fire (M4), or the scheduler to tick (M7) is out of scope.
M1 is purely about having a working skeleton.
