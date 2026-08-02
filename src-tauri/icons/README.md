# Icons

The required desktop and mobile icon variants are already generated in this
directory:

- `32x32.png`
- `128x128.png`
- `128x128@2x.png` (256×256)
- `icon.icns` (macOS)
- `icon.ico` (Windows)
- `icon.png` (used by the tray on Linux)

## Generating icons

Use [tauri-icon](https://github.com/tauri-apps/tauri/tree/dev/tooling/cli#icon-generator):

```bash
npm run tauri icon path/to/source-1024x1024.png
```

This will generate all required sizes and formats from a single 1024×1024
PNG with transparency.

Regenerate them only when the source artwork changes.
