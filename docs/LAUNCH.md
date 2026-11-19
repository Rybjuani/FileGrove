# Lanzamiento y prueba de FileGrove

El punto de entrada canónico del repositorio es:

- Windows: `filegrove.cmd` (doble clic o desde PowerShell).
- Linux: `./filegrove.sh`.

Los lanzadores ejecutan el binario release empaquetado cuando existe y, si no,
inician el modo de desarrollo con `npm run tauri:dev`. Ya no ejecutan un binario
debug aislado, porque ese binario no incluye el frontend y no puede arrancar sin
Vite. Después de cambiar el código hay que reconstruir el release; de lo
contrario, el lanzador sigue abriendo la compilación anterior.

Para forzar el modo de desarrollo: `filegrove.cmd --dev` o
`./filegrove.sh --dev`.

Para instalar un icono fijo en el menú de aplicaciones y en el Escritorio de
Linux, ejecutar una vez `./scripts/install-desktop-launcher.sh`. El acceso
directo apunta al lanzador canónico y no depende de una terminal abierta.

## Instalador de Windows

El artefacto actual de prueba es un instalador NSIS por usuario:

```text
src-tauri\target\release\bundle\nsis\FileGrove_0.1.0_x64-setup.exe
```

- No requiere permisos de administrador.
- Se instala por defecto en `%LOCALAPPDATA%\FileGrove`.
- Usa WebView2; si falta el runtime, el instalador lo descarga y necesita
  conexión a internet.
- No está firmado digitalmente. Windows SmartScreen puede mostrar una
  advertencia. Verificar el SHA-256 antes de ejecutarlo.
- El desinstalador permite conservar los datos de la aplicación. Dejar
  desmarcada la opción de borrar datos si se quiere mantener la configuración,
  las reglas, el historial y el Undo.

La guía completa de la fase de pruebas está en
[`TESTING_WINDOWS.md`](TESTING_WINDOWS.md).

## Perfil predeterminado

En el primer arranque de una base de datos nueva FileGrove crea un perfil
idempotente para las carpetas estándar del usuario. Las instalaciones que ya
tienen carpetas configuradas se conservan intactas:

- Escritorio: sólo archivos directamente dentro del Escritorio; no entra en
  repositorios ni en ninguna subcarpeta.
- Downloads: sólo archivos directamente dentro de Downloads; no entra en
  subcarpetas.
- Documentos, imágenes y vídeos: se mueven a las bibliotecas estándar del
  sistema del usuario.

El perfil sólo habilita movimientos para documentos, imágenes y vídeos. Usa
renombrado ante conflictos, conserva duplicados y no activa borrado permanente.
Cada movimiento pasa por el journal de Undo existente.

El perfil también activa el inicio con Windows y marca el onboarding como
completado. Para una prueba sin sorpresas, usar una cuenta de Windows separada,
pausar todo antes de copiar archivos reales, o quitar las carpetas creadas
automáticamente.

La primera ejecución sobre datos reales debe revisarse en Activity y comenzar
con archivos de prueba. Para pausar todo, usar el control de pausa de FileGrove;
la pausa queda guardada incluso si la aplicación se reinicia.

## Inicio con Windows

El backend reconcilia la preferencia `launchAtStartup` mediante el plugin de
autostart de Tauri. El perfil inicial la activa. Se puede desactivar desde
Settings; no hace falta crear accesos directos manualmente.

## Datos y recuperación

- Windows: `%APPDATA%\app.filegrove.desktop\app.db` (puede haber `-wal` y
  `-shm` mientras la aplicación está abierta).
- Linux: `~/.local/share/app.filegrove.desktop/app.db`, o
  `$XDG_DATA_HOME/app.filegrove.desktop/app.db` si `XDG_DATA_HOME` está
  definido.

Cerrar la ventana puede ocultar la aplicación sin terminar el proceso. Para
copiar o restaurar la base de datos hay que elegir **Quit** en el menú de la
bandeja y confirmar que no queda ningún proceso `filegrove`.

Los movimientos se recuperan con **Undo latest**. Los elementos enviados a la
Papelera o al Recycle Bin se recuperan desde el sistema operativo. El borrado
permanente y los comandos externos no tienen recuperación de FileGrove.
