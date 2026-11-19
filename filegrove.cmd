@echo off
setlocal
set "REPO=%~dp0"
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%REPO%scripts\filegrove.ps1" %*
exit /b %ERRORLEVEL%
