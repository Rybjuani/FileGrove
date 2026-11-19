#!/usr/bin/env bash
set -euo pipefail
repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

if [[ -x "$repo_dir/src-tauri/target/release/filegrove" ]]; then
  exec "$repo_dir/src-tauri/target/release/filegrove" "$@"
fi
if [[ -x "$repo_dir/src-tauri/target/debug/filegrove" ]]; then
  exec "$repo_dir/src-tauri/target/debug/filegrove" "$@"
fi

cd "$repo_dir"
exec npm run tauri:dev -- "$@"
