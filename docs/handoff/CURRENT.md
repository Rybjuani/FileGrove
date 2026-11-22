# Handoff — estado actual

**Fecha:** 2026-11-22
**Rama:** `fix/beta-reliability` (desde `linux-recovery`)

## Estado

T5 completa, moves exFAT verificados.

## Hecho

- Commits 1-5 (T1-T4): T1 logs a archivo, T2 dump de actividad + reporte,
  T3 forense de rutas, T4 corrección de la evidencia + fix de
  `component_starts_with` (boundaries de drive + des-verbatim) con tests.
- T5:
  - Commit A (`9b54f30`):
    - `src-tauri/src/engine/file_ops.rs`: helper `publish_no_replace` (hard link
      no-replace; si el FS no soporta, chequeo carreras y `fs::rename`
      same-volume, negándose a pisar destino existente). `copy_publish_then_remove`
      ahora usa el fallback y solo borra el temp si sigue existiendo (rename lo
      consume). `publish_hard_link` se conserva para `restore_retained_backup`.
    - `src-tauri/src/engine/executor.rs`: warn nuevo `move execution failed` con
      el error OS completo (`{error:#}`) antes de `action_error` (visible para
      diagnósticos futuros de tipos de error tipo los 21 `MOVE_TEMP_CREATE_FAILED`).
    - Tests nuevos en `file_ops.rs`: `publish_no_replace_refuses_existing_destination`
      y `publish_no_replace_publishes_when_absent` (pasan).
    - Harness `src-tauri/examples/verify_move_exfat.rs` (API pública de
      `integration_test_support`, sin tocar `F:\Descargas` ni la DB de producción).
  - Commit B: verificación aislada en `F:\fg-t5-test` (dir + DB throwaway):
    - a.jpg / b.txt / c.mp3: **3/3 success=true, movidos a `Sorted/`**,
      sin `MOVE_PUBLISH_FAILED` ni `MOVE_TEMP_CREATE_FAILED`.
    - Reporte: `docs/diagnostics/2026-11-22-exfat-move.md`.
    - Limpieza: `F:\fg-t5-test` borrado completo, sin rastro.
- Gate: cargo test (73 Rust + 1 integración) y npm typecheck + 17 tests, en verde.

## Pendiente

- Revisión del orquestador de la serie fix/beta-reliability (T1-T5).
- Si reaparecen `MOVE_TEMP_CREATE_FAILED`, el warn nuevo de executor ya captura
  el error OS completo en el log.
- NO tocar `migrate_duplicate_quick_rules`.

## Blockers

- **Entorno (preexistente):** `cargo test --test watcher_service` no arranca en
  esta máquina (`STATUS_ENTRYPOINT_NOT_FOUND`, 0xc0000139, DLL del entorno,
  reproducido en `linux-recovery` limpio). Todo lo demás del gate en verde.

## Próxima tarea

- Orquestador: revisión de la serie / nueva tarjeta (queda claro que los moves
  funcionan sobre exFAT con el pipeline actual).
