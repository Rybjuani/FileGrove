[CmdletBinding()]
param(
  [Parameter(ValueFromRemainingArguments = $true)]
  [string[]] $ArgumentList
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$candidates = @(
  (Join-Path $repo 'src-tauri\target\release\filegrove.exe'),
  (Join-Path $repo 'src-tauri\target\debug\filegrove.exe')
)
$binary = $candidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1

if ($binary) {
  Start-Process -FilePath $binary -WorkingDirectory $repo -ArgumentList $ArgumentList
  exit 0
}

if (-not (Get-Command npm.exe -ErrorAction SilentlyContinue)) {
  throw 'No se encontró FileGrove compilado ni npm. Ejecutá npm install y npm run tauri:build.'
}

Push-Location $repo
try {
  & npm.exe run tauri:dev -- $ArgumentList
  exit $LASTEXITCODE
}
finally {
  Pop-Location
}
