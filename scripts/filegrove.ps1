 $ErrorActionPreference = 'Stop'
 $repo = Split-Path -Parent $PSScriptRoot
 $candidates = @(
(Join-Path $repo 'src-tauri\target\release\filegrove.exe'),
(Join-Path $repo 'src-tauri\target\debug\filegrove.exe')
)
 $binary = $candidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
if ($binary) {
if ($args.Count -gt 0) {
Start-Process -FilePath $binary -WorkingDirectory $repo -ArgumentList @args
} else {
Start-Process -FilePath $binary -WorkingDirectory $repo
}
exit 0
}
 $npm = Get-Command npm.cmd -ErrorAction SilentlyContinue
if (-not $npm) {
 $npm = Get-Command npm -ErrorAction SilentlyContinue
}
if (-not $npm) {
throw 'No se encontró FileGrove compilado ni npm. Ejecutá npm install y npm run tauri:build.'
}
Push-Location $repo
try {
if ($args.Count -gt 0) {
& $npm.Source run tauri:dev -- @args
} else {
& $npm.Source run tauri:dev
}
exit $LASTEXITCODE
}
finally {
Pop-Location
}