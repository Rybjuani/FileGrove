# Lanzamiento persistente de FileGrove

El punto de entrada canónico del repositorio es:

- Windows: `filegrove.cmd` (doble clic o desde PowerShell).
- Linux: `./filegrove.sh`.

Para instalar un icono fijo en el menú de aplicaciones y en el Escritorio de
Linux, ejecutar una vez `./scripts/install-desktop-launcher.sh`. El acceso
directo apunta al lanzador canónico y no depende de una terminal abierta.

Los lanzadores buscan primero el binario release, luego el binario debug y sólo
como último recurso ejecutan `npm run tauri:dev`. Así, una compilación nueva no
rompe el modo de iniciar la aplicación y siempre se ejecuta desde la raíz del
repositorio.

## Perfil predeterminado

En el primer arranque de una base de datos nueva FileGrove crea un perfil
idempotente para las carpetas estándar del usuario. Las instalaciones que ya
tienen carpetas configuradas se conservan intactas:

- Escritorio: sólo archivos directamente dentro del Escritorio; no entra en
  repositorios ni en ninguna subcarpeta.
- Downloads: permite subcarpetas hasta 64 niveles.
- Documentos, imágenes y vídeos: se mueven a las bibliotecas estándar del
  sistema del usuario.

El perfil sólo habilita movimientos para documentos, imágenes y vídeos. Usa
renombrado ante conflictos, conserva duplicados y no activa borrado permanente.
Cada movimiento pasa por el journal de Undo existente.

La primera ejecución sobre datos reales debe revisarse en Activity y comenzar
con archivos de prueba. Para pausar todo, usar el control de pausa de FileGrove;
la pausa queda guardada incluso si la aplicación se reinicia.

## Inicio con Windows

El backend ya reconcilia la preferencia `launchAtStartup` mediante el plugin de
autostart de Tauri. El perfil inicial la activa. Se puede desactivar desde
Settings; no hace falta crear accesos directos manualmente.

Windows todavía requiere una compilación Tauri nativa para producir
`src-tauri/target/release/filegrove.exe`; el repositorio no debe confundirse con
un instalador Windows ya validado.
