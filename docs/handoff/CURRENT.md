# Handoff — estado actual

**Fecha:** 2026-11-22
**Rama:** `fix/beta-reliability` (desde `linux-recovery`)

## Estado

T6 completa, gate de escaneo relajado, verificado.

## Hecho

- T1-T5: ver commits anteriores de esta serie (historia resumida en
  `docs/diagnostics/2026-11-22*.md`): logs a archivo, dump de actividad,
  forense de rutas, fix `component_starts_with` (boundaries + verbatim),
  fallback de publish para exFAT, verificados (moves OK en F: real).
- T6 (commit `7428fe7`):
  - `src-tauri/src/ipc/files.rs` (`organize_inner_with_gate`): los aborts
    `SCAN_LIMIT_REACHED` / `SCAN_INCOMPLETE` se reemplazaron por warnings que
    no detienen la corrida — se organiza el subconjunto legible (la seguridad
    sigue por archivo: `validate_source_file` + `revalidate_immediately_before_action`).
    La respuesta ahora incluye `scanTruncated` / `scanUnreadable` en `data`
    (listos para aviso UI futuro). `emit_organize_error` queda para el branch
    de scan fallido.
  - Verificación aislada en `F:\fg-t6-test` (DB throwaway + harness temporario
    de 4 archivos, borrado después): 4/4 movidos a `Sorted/` con `DONE=4`,
    sin aborts de scan en el log del día. Boot de `tauri dev` con el código
    nuevo OK. El camino `errors > 0` no se forzó en runtime (difícil sobre
    exFAT sin archivos rotos); verificado por revisión de código.
  - Reporte: `docs/diagnostics/2026-11-22-scan-gate.md`.
  - Limpieza completa (`fg-t6-test` y harness temporario fuera). DB de
    producción y `F:\Descargas` intactas en esta tarjeta.

## Pendiente

- Revisión del orquestador de la serie fix/beta-reliability (T1-T6).
- Si reaparecen `MOVE_TEMP_CREATE_FAILED`, el warn de executor ya captura el
  error OS completo en el log.
- NO tocar `migrate_duplicate_quick_rules`.

## Blockers

- **Entorno (preexistente):** `cargo test --test watcher_service` no arranca en
  esta máquina (`STATUS_ENTRYPOINT_NOT_FOUND`, 0xc0000139, DLL del entorno,
  reproducido en `linux-recovery` limpio). Todo lo demás del gate en verde.

## Próxima tarea

- Orquestador: revisión de la serie / nueva tarjeta.
