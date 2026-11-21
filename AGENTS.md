# FileGrove — reglas de agente

Reglas obligatorias para todo agente de mantenimiento que trabaje en este repo.

## Alcance

- Una tarjeta = un commit = un problema. Diff < 200 líneas salvo indicación explícita.
- Prohibido: dependencias nuevas, cambios de arquitectura, mejoras no pedidas, force-push.

## Gate de calidad

Antes de commitear cualquier cambio de código, todos los checks deben pasar en verde:

```bash
cd src-tauri && cargo test
npm run typecheck && npm test
```

## Si algo falla

- Si no podés completar algo, NO commitees código roto: escribí el blocker en
  `docs/handoff/CURRENT.md` y commiteá solo eso.

## Handoff

- Actualizá `docs/handoff/CURRENT.md` en cada commit: estado, hecho, pendiente, blockers, próxima tarea.

## Ramas y push

- Trabajá en la rama `fix/beta-reliability` (creada desde `linux-recovery`).
- Pusheá cada commit a medida que lo hacés.
