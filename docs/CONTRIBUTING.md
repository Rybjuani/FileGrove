# Contributing to FileGrove

Thanks for your interest! This document covers the basics.

## Quick start

```bash
npm install
npm run tauri:dev
```

Run those commands from the local FileGrove repository. The project is still in
private release preparation and does not publish end-user releases yet.

For prerequisites (Rust, Node 20+, system libs), see the
[Tauri 2 getting started guide](https://tauri.app/v2/guides/getting-started/prerequisites/).

## Project layout

See `docs/SPEC.md` §3.3 for the full layout. The short version:

- `src-tauri/` — Rust backend (rules, watcher, storage, IPC)
- `src/` — React frontend (pages, components, stores, i18n)
- `docs/` — Specs and architecture docs
- `tests/unit/` — Frontend unit tests
- `src-tauri/tests/` — Black-box Rust integration tests

## Development workflow

1. Pick one of the pending items in `docs/ROADMAP_1_0.md` or the ordered
   checklist in `docs/HANDOFF_2026-08-08.md`.
2. Branch: `git checkout -b feat/short-description`.
3. Code. Run checks locally:
   ```bash
   npm run typecheck && npm run lint && npm run test -- --run
   cd src-tauri && cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test
   ```
4. Commit using
   [conventional commits](https://www.conventionalcommits.org/):
   `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`.
5. Push the feature branch, open a PR, and require all checks to pass before
   merge. Maintainer-directed recovery work may land directly on
   `linux-recovery` only when it is staged explicitly, atomic, and followed by
   green CI.

## Code style

- **Rust**: `cargo fmt`, no clippy warnings, idiomatic error handling
  with `anyhow`/`thiserror`.
- **TypeScript**: strict mode, no `any` without justification,
  `eslint --max-warnings 0`.
- **Commits**: 72-char subject, imperative mood, body explains _why_
  not _what_.

## Tests

- **Frontend unit** (`tests/unit/`): fast, isolated, run on every push.
- **Rust unit** (`src-tauri/src/**`): engine and persistence behavior beside
  the code it covers.
- **Rust integration** (`src-tauri/tests/`): exercise production services via
  Tauri's headless test harness and disposable directories.
- **E2E**: Playwright coverage will be added before the release candidate.

Aim for at least one test per IPC handler and per rule-engine branch.

## Translations

Locale files live in `src/i18n/locales/`. To add a language:

1. Copy `en.json` to `<lang>.json`.
2. Translate the values.
3. Register it in `src/i18n/index.ts`.

## Reporting bugs

Record a local bug report with:

- FileGrove version (`About` page)
- OS + version
- Steps to reproduce
- Expected vs actual behavior
- Redacted terminal diagnostic output (the current build does not create a
  persistent FileGrove log file by default)

## Code of conduct

Be kind. Be patient. Be helpful. We're all volunteering our time.

## License

By contributing, you agree that your contributions will be licensed
under the Apache License 2.0.
