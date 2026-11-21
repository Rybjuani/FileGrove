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
- El instalador no incluye carpetas, reglas ni rutas del equipo donde se
  compiló: la configuración se crea en el primer arranque de cada equipo.

La guía completa de la fase de pruebas está en
[`TESTING_WINDOWS.md`](TESTING_WINDOWS.md).

## Primer inicio y automatizacion

Una instalacion nueva no agrega carpetas vigiladas ni reglas, no organiza
archivos en segundo plano y no activa el inicio con Windows. La aplicacion
muestra el onboarding y espera a que el usuario agregue una carpeta.

Las carpetas nuevas empiezan en modo Manual y detenidas. El usuario puede
revisar una Vista previa y organizar manualmente; el modo Automatico se activa
solo cuando el usuario lo elige. El inicio con Windows es una preferencia
independiente y tambien queda desactivada por defecto.

Al actualizar una instalacion existente, FileGrove conserva las carpetas,
reglas y preferencias ya guardadas.

## Inicio con Windows

El backend reconcilia la preferencia `launchAtStartup` mediante el plugin de
autostart de Tauri. En una instalacion nueva la preferencia queda desactivada;
el usuario puede habilitarla desde Settings.

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

La Papelera de Windows es por volumen: un archivo eliminado desde `F:` se
conserva en `F:\$RECYCLE.BIN` y no consume espacio en `C:`. FileGrove no
redirige ni reubica la Papelera del sistema.
