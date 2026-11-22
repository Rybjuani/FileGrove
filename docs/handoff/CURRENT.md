# Handoff — estado actual

**Fecha:** 2026-11-22
**Rama:** `fix/beta-reliability` (desde `linux-recovery`)

## Estado

T4 completa (commits A y B). Verificación en disco real F: realizada en parte —
el fallo `SOURCE_FILE_OUTSIDE_WATCHED_FOLDER` está arreglado, pero la verificación
encontró un blocker NUEVO para completar moves end-to-end: `MOVE_PUBLISH_FAILED`
(hard links no soportados por exFAT). Requiere tarjeta T5.

## Hecho

- Commits 1-4 (T1-T3): ver historia en esta propia sección en commits previos.
- Commit A (`77ec959`): corregido `inspect_paths.rs` para replicar la comparación
  REAL (`key == root || key.starts_with(&root)`), sin separadores inventados.
  Resultado: veredicto real en F:\ es `true` para raíz + 3/3 archivos. La hipótesis
  "doble backslash" de T3 era incorrecta. Documentado en
  `docs/diagnostics/2026-11-22-paths.md` (sección "Corrección T4").
- Commit B (`d5fbfde`): `component_starts_with` (Windows) reescrita con
  `normalize_windows_key` (des-verbatim `\\?\`, UNC equivalente) y límite por
  componente con manejo correcto de raíces de drive (la clave `f:\` ya no impone
  un prefijo imposible).
  Test nuevo `component_starts_with_drive_root_boundaries_and_verbatim` (7 asserts,
  pasa). Versiones de `#[cfg(not(windows))]` sin tocar.
- Verificación en F: real (agente, sin clic de usuario):
  - `npm run tauri dev` compiló y abrió ventana (dev). El layer de archivo (T1)
    logs: `%APPDATA%\app.filegrove.desktop\logs\filegrove.log.2026-11-22` OK.
  - Con watcher catch-up sobre `F:\Descargas` (recursivo): **0 coincidencias
    nuevas `SOURCE_FILE_OUTSIDE_WATCHED_FOLDER`** (32087 queda intacto): el fix B
    pasa la validación. +1656 `MOVE_PUBLISH_FAILED` y +21 `MOVE_TEMP_CREATE_FAILED`.
  - Causa del nuevo fallo: `publish_hard_link` (`engine/file_ops.rs:380`) usa
    `fs::hard_link`; **exFAT no soporta hard links** → cada move pasa la
    validación y la copia temporal, pero falla al publicar.
  - Archivos reales del usuario: intactos (1677 intentos, todos fallidos, ninguna
    row de éxito). Sin `move-temp` huérfanos. Config restaurada a su estado previo
    (`automationPaused=true`, `watcher_status=paused`). Archivos de prueba
    (`fg-verify-*`) eliminados. Patch de debug temporal en `ipc/files.rs`
    revertido, no committed.

## Pendiente

- **T5 (nueva):** fallback de publish para volúmenes sin hard links — en
  `publish_then_remove_source` / `copy_publish_then_remove`
  (`src-tauri/src/engine/file_ops.rs`): cuando el hard link falle por
  filesystem sin soporte, usar `fs::rename` same-volume (con rename-backup si
  existe destino) o copy+replace. Investigar también las 21 `MOVE_TEMP_CREATE_FAILED`.
- NO tocar `migrate_duplicate_quick_rules`.

## Blockers

- **T5 (funcional, nuevo):** moves sobre exFAT (disco externo F:) fallan en
  publish porque exFAT no soporta hard links. Evidencia: log del 2026-11-22 y
  activity DB (1656 + 21 intentos fallidos). Sin T5, con un disco externo
  exFAT la app no puede organizar nada.
- **Entorno (preexistente):** `cargo test --test watcher_service` no arranca en
  esta máquina (`STATUS_ENTRYPOINT_NOT_FOUND`, 0xc0000139, DLL del entorno,
  reproducido en `linux-recovery` limpio). Todo lo demás del gate está en verde:
  71 tests Rust (incluye el nuevo de boundaries) + frontend (typecheck + 17 tests).

## Próxima tarea

- T5: publish fallback para exFAT en `file_ops.rs` (orquestador).
