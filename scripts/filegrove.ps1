$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot

# Only release binaries bundle the frontend and can run standalone without Vite.
$releaseBinary = Join-Path $repo 'src-tauri\target\release\filegrove.exe'

# If a release build exists and no arguments were passed asking for dev, run the standalone release app.
if ((Test-Path -LiteralPath $releaseBinary) -and (-not ($args -contains '--dev'))) {
    if ($args.Count -gt 0) {
        Start-Process -FilePath $releaseBinary -WorkingDirectory $repo -ArgumentList @args
    } else {
        Start-Process -FilePath $releaseBinary -WorkingDirectory $repo
    }
    exit 0
}

# Otherwise run the development server (Vite + Tauri)
$npm = Get-Command npm.cmd -ErrorAction SilentlyContinue
if (-not $npm) {
    $npm = Get-Command npm -ErrorAction SilentlyContinue
}
if (-not $npm) {
    throw 'No se encontró FileGrove compilado ni npm. Ejecutá npm install y npm run tauri:build.'
}

Push-Location $repo
try {
    $devArgs = @($args | Where-Object { $_ -ne '--dev' })
    if ($devArgs.Count -gt 0) {
        & $npm.Source run tauri:dev -- @devArgs
    } else {
        & $npm.Source run tauri:dev
    }
    exit $LASTEXITCODE
}
finally {
    Pop-Location
}
