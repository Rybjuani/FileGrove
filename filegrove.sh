#!/usr/bin/env bash
set -euo pipefail
repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

# Only release binaries bundle the frontend and can run standalone without Vite.
if [[ -x "$repo_dir/src-tauri/target/release/filegrove" ]] && [[ "${1:-}" != "--dev" ]]; then
  exec "$repo_dir/src-tauri/target/release/filegrove" "$@"
fi

cd "$repo_dir"
exec npm run tauri:dev -- "$@"
