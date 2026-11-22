# Verificación de moves en exFAT — 2026-11-22 (T5)

Verificación aislada: `src-tauri/examples/verify_move_exfat.exe` contra
`F:\fg-t5-test` (directorio throwaway, DB throwaway `verify.db`, archivos
desechables generados por el propio harness). La DB de producción
(`%APPDATA%\app.filegrove.desktop\app.db`) no fue tocada en esta tarjeta y
`F:\Descargas` y demás carpetas preexistentes quedaron intactas.

## Resultado

| Archivo | success | error | new_path |
| --- | --- | --- | --- |
| a.jpg | true | None | `F:\fg-t5-test\Sorted\a.jpg` |
| b.txt | true | None | `F:\fg-t5-test\Sorted\b.txt` |
| c.mp3 | true | None | `F:\fg-t5-test\Sorted\c.mp3` |

- 3/3 movidos a la subcarpeta `Sorted/` según la regla (`move` → `Sorted`,
  `ConflictBehavior::Rename`, `DuplicateBehavior::KeepBoth`).
- `MOVE_PUBLISH_FAILED`: **no reproducido**. En el disco exFAT el
  `fs::hard_link` falla como siempre, pero ahora `publish_no_replace` hace el
  fallback a `fs::rename` same-volume y el move se completa.
- `MOVE_TEMP_CREATE_FAILED`: **no reproducido** en esta corrida (los 21 casos
  de la corrida T4 eran del binario sin el warn `{error:#}`; quedan sin causa
  OS conocida — si vuelven a aparecer, el warn nuevo en
  `executor.rs` grabará el detalle del error OS en el log).
- DB throwaway: 3 filas en `activity` con `success=1, error=None`.
- Estado final de `F:\fg-t5-test` antes de la limpieza: solo `Sorted/` con los
  3 archivos movidos y `verify.db` (sin archivos remanentes en la raíz, sin
  `move-temp`).

## Limpieza

`F:\fg-t5-test` eliminado completo (archivos, `Sorted/` y `verify.db`). Sin
rastro en el disco externo.

## Conclusión

El pipeline completo de move (validación de fuente T4 + publicación T5) queda
operativo sobre exFAT. Moves destino NTFS siguen por el camino de hard link.
