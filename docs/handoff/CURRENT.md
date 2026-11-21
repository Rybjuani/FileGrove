# Handoff — estado actual

**Fecha:** 2026-11-22
**Rama:** `fix/beta-reliability` (desde `linux-recovery`)

## Estado

T1 (logs a archivo) implementada, gate con una salvedad de entorno (ver Blockers).

## Hecho

- Commit 1: rama `fix/beta-reliability`, `AGENTS.md`, protocolo de handoff.
- T1:
  - `src-tauri/src/lib.rs`: `init_tracing()` ahora agrega un segundo fmt layer que
    escribe a archivo con rotación diaria en `<data_dir>/app.filegrove.desktop/logs/filegrove.log`
    vía `tracing-appender` (dep existente). El dir se resuelve con `directories::BaseDirs`
    porque `init_tracing` corre antes de tener `AppHandle` (mismo patrón que
    `paths::safety`). Si el directorio no se puede crear, queda sólo stdout y la app
    arranca igual. El fmt layer de stdout se conserva.
  - `src-tauri/src/engine/executor.rs`: `record_activity()` ahora emite
    `tracing::warn!(source, destination, error_code, "move failed")` para cada move
    fallido (por `Action::Move`, sin registrar skips).
- Ruta `src-tauri/examples/` ya existía (untracked, `inspect_db.rs`); no la toqué en este commit.

## Pendiente

- T2: herramienta `src-tauri/examples/dump_activity.rs`, corrida contra la DB real,
  reporte de diagnóstico `docs/diagnostics/2026-11-22.md`.

## Blockers

- **Entorno (preexistente, no introducido por T1):** en esta máquina Windows el
  binario de integración `watcher_service` (`cargo test --test watcher_service`)
  no llega a arrancar: falla con `STATUS_ENTRYPOINT_NOT_FOUND` (0xc0000139), un
  problema de resolución de DLL del entorno. Reproducido idéntico en
  `linux-recovery` sin cambios locales (verificado con stash). El resto de la
  suite Rust (87 tests: unitarios + `conflict_policies`) y el gate de frontend
  (`npm run typecheck && npm test`, 17 tests) están en verde.

## Próxima tarea

- Commit 3 (T2): diagnóstico sin tocar la app.
