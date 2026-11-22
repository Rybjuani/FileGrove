# Handoff — estado actual

**Fecha:** 2026-11-22
**Rama:** `fix/beta-reliability` (desde `linux-recovery`)

## Estado

T8 completa, sesiones del watcher visibles, pendiente check visual del usuario.

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
- T7 (frontend-only):
  - `src/lib/locale.ts` nuevo: `detectUiLocale` (es*/lowercase → 'es', resto
    'en') y `firstRunLocalePatch` (solo cuando `onboardingCompleted !== true`;
    siembra idioma del SO y `theme: 'system'`, sin tocar lo ya configurado).
  - `src/i18n/index.ts`: `lng` inicial detectado con `detectUiLocale(navigator.language)`;
    `fallbackLng: 'en'` sin cambios.
  - `src/App.tsx`: seed de primer arranque con `firstRunSeeded` (useRef, una
    sola vez) que aplica el patch vía `updateSettings` y cambia el idioma de
    i18n. Único ajuste sobre el código indicado: cast `patch as Partial<SettingsPatch>`
    porque `Partial<Settings>` lleva index signature y `FirstRunPatch` no
    (import con alias para no chocar con la página `Settings`).
  - Tests: `tests/unit/locale.test.ts` (6 casos: variantes es, fallback en/undefined/
    null/'', seed en primer arranque, no-op tras onboarding, no-op ya configurado,
    null settings). 23 tests frontend en verde.
  - La lógica queda cubierta por unit tests puros; sin DB fresca de producción
    ni render de App (cableado validado por typecheck).
- T8 (frontend-only, `src/components/FolderCard.tsx`):
  - Listener `organize:done` nuevo: limpia sessión/busy/banner de error pegado,
    setea resultado (exitoso/parcial/cancelado/error legible) y refresca las
    sesiones de undo cuando `done > 0`.
  - `onStarted` ya no filtra por `busyRef` (el watcher ahora dispara también
    sesiones no iniciadas por el botón — automatización del watcher).
  - `handleOrganize`: si hay sesión activa, cancela; el éxito ya no se maneja
    en la respuesta del IPC (la maneja `organize:done` del listener). El banner
    de "error pegado" se limpia en started/done vía `clearError`.
  - Barra de progreso aparece cuando `activeSessionId && progress`.
  - Botón Organizar/Cancelar según sesión activa (ícono Square/Play).
  - Tipos: `OrganizeDone` ya declaraba `error?`/`cancelled`/`stats` — sin cambio.
  - Gate: typecheck limpio, 23 tests frontend, cargo test 74 ok (fallo de entorno
    `watcher_service` preexistente), `npm run build` OK.
  - Verificación UI no posible por el agente (no ve el webview): chequeo visual
    de 10 segundos pendiente en el usuario (carpeta Automática + soltar archivo
    → barra de progreso y botón Cancelar; al terminar se limpia el banner).

## Pendiente

- Revisión del orquestador de la serie fix/beta-reliability (T1-T8).
- Si reaparecen `MOVE_TEMP_CREATE_FAILED`, el warn de executor ya captura el
  error OS completo en el log.
- NO tocar `migrate_duplicate_quick_rules`.

## Blockers

- **Entorno (preexistente):** `cargo test --test watcher_service` no arranca en
  esta máquina (`STATUS_ENTRYPOINT_NOT_FOUND`, 0xc0000139, DLL del entorno,
  reproducido en `linux-recovery` limpio). Todo lo demás del gate en verde.

## Próxima tarea

- Orquestador: revisión de la serie / nueva tarjeta.
