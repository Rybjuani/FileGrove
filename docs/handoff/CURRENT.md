# Handoff — estado actual

**Fecha:** 2026-11-22
**Rama:** `fix/beta-reliability` (desde `linux-recovery`)

## Estado

T3 completa, esperando fix T4 del orquestador.

## Hecho

- Commit 1: rama `fix/beta-reliability`, `AGENTS.md`, protocolo de handoff.
- Commit 2 (T1):
  - `src-tauri/src/lib.rs`: `init_tracing()` agrega un layer que escribe a archivo
    con rotación diaria en `<data_dir>/app.filegrove.desktop/logs/filegrove.log`
    vía `tracing-appender`, resuelto con `directories::BaseDirs` (sin `AppHandle`),
    conservando el fmt layer de stdout. Si no se puede crear el directorio, queda
    stdout solo y la app arranca igual.
  - `src-tauri/src/engine/executor.rs`: `record_activity()` emite
    `tracing::warn!(source, destination, error_code, "move failed")` para cada
    move fallido.
- Commit 3 (T2):
  - `src-tauri/examples/dump_activity.rs`: abre la SQLite pasada por argv[1]
    (solo lectura, `rusqlite` existente) e imprime el top 20 de errores de
    actividad fallida, el total de filas y `settings.language`.
  - Ejecutado contra `%APPDATA%\app.filegrove.desktop\app.db`: 32087 filas de
    actividad, el 100% de los fallos es `SOURCE_FILE_OUTSIDE_WATCHED_FOLDER` (32087),
    `settings.language = "es"`.
  - `Get-Volume -DriveLetter F`: existe, `FileSystemType = Unknown`,
    `SizeRemaining ≈ 313.5 GiB`.
  - Reporte en `docs/diagnostics/2026-11-22.md` (solo códigos y agregados, sin
    datos personales).
- Commit 4 (T3):
  - `src-tauri/examples/inspect_paths.rs`: imprime raw | std canonicalize |
    dunce canonicalize | key | veredicto para la raíz y 3 archivos de la raíz
    vigilada pasada por argv[1]. Sin panic: errores como `ERR: <motivo>`.
  - Ejecutado contra `F:\` (ver reporte `docs/diagnostics/2026-11-22-paths.md`):
    namespace verbatim-disk (`\\?\F:\`, volver a `F:\` con dunce), veredicto
    `false` para 4/4 archivos.
  - Causa raíz evidenciada: `component_starts_with` falla cuando la raíz
    vigilada es la raíz de un drive — `canonical_path_key` de `F:\` termina en
    backslash y el test impone prefijo `f:\\` (doble backslash), imposible de
    matchear. No se tocó `safety.rs` (solo evidencia).

## Pendiente

- T4 (orquestador): fix del caso borde de `component_starts_with` para raíces
  de drive (`src-tauri/src/paths/safety.rs:220`). Sugerencia en el reporte:
  normalizar la clave de raíz sin backslash final antes de comparar o comparar
  por componentes. NO tocar `migrate_duplicate_quick_rules`.

## Blockers

- **Entorno (preexistente, no introducido por esta tarjeta):** en esta máquina
  Windows el binario de integración `watcher_service`
  (`cargo test --test watcher_service`) no llega a arrancar: falla con
  `STATUS_ENTRYPOINT_NOT_FOUND` (0xc0000139), problema de resolución de DLL del
  entorno. Reproducido idéntico en `linux-recovery` sin cambios locales (verificado
  con stash). El resto de la suite Rust (71 tests) y el gate de frontend
  (`npm run typecheck && npm test`, 17 tests) están en verde.

## Próxima tarea

- T4: fix del caso borde de `component_starts_with` para raíces de drive
  (esperando al orquestador).
