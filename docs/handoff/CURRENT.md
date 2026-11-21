# Handoff — estado actual

**Fecha:** 2026-11-22
**Rama:** `fix/beta-reliability` (desde `linux-recovery`)

## Estado

Inicial: protocolo de orquestación y handoff creado.

## Hecho

- Rama `fix/beta-reliability` creada desde `linux-recovery`.
- `AGENTS.md` con reglas de mantenimiento (una tarjeta = un commit, gate de tests, prohibiciones).
- Este archivo de handoff inicial.

## Pendiente

- T1: logs a archivo (tracing-appender rolling diario en `<app_data_dir>/logs/filegrove.log`) + `tracing::warn!` en moves fallidos.
- T2: herramienta de diagnóstico `src-tauri/examples/dump_activity.rs` + reporte `docs/diagnostics/2026-11-22.md`.

## Blockers

- Ninguno por ahora.

## Próxima tarea

- Commit 2 (T1): logs a archivo y warn en moves fallidos.
