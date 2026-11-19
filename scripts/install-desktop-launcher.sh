#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
launcher="$repo_dir/filegrove.sh"
icon="$repo_dir/src-tauri/icons/128x128.png"
app_name="FileGrove"

if [[ ! -x "$launcher" ]]; then
  chmod +x "$launcher"
fi

applications_dir="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
mkdir -p "$applications_dir"

desktop_entry="[Desktop Entry]
Type=Application
Name=FileGrove
Comment=Organización segura de archivos
Exec=$launcher
Icon=$icon
Terminal=false
Categories=Utility;FileTools;
StartupWMClass=filegrove
"

printf '%s' "$desktop_entry" > "$applications_dir/filegrove.desktop"
chmod 644 "$applications_dir/filegrove.desktop"

desktop_dir=""
if command -v xdg-user-dir >/dev/null 2>&1; then
  desktop_dir="$(xdg-user-dir DESKTOP 2>/dev/null || true)"
fi
if [[ -z "$desktop_dir" || ! -d "$desktop_dir" ]]; then
  for candidate in "$HOME/Escritorio" "$HOME/Desktop"; do
    if [[ -d "$candidate" ]]; then
      desktop_dir="$candidate"
      break
    fi
  done
fi

if [[ -n "$desktop_dir" && -d "$desktop_dir" ]]; then
  printf '%s' "$desktop_entry" > "$desktop_dir/FileGrove.desktop"
  chmod +x "$desktop_dir/FileGrove.desktop"
fi

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$applications_dir" >/dev/null 2>&1 || true
fi

printf 'FileGrove instalado en el menú de aplicaciones.\n'
if [[ -n "$desktop_dir" && -d "$desktop_dir" ]]; then
  printf 'Acceso directo: %s\n' "$desktop_dir/FileGrove.desktop"
fi
