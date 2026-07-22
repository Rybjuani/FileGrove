# Icons

Place the following files in this directory before building:

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

## Placeholder

Until you provide real artwork, the build will fail. As a stopgap, use a
simple solid-color square:

```bash
# Generate a 1024x1024 dark-green square
convert -size 1024x1024 xc:'#0f172a' -gravity center -fill '#10b981' -annotate +0+0 'FG' icon-source.png
npm run tauri icon icon-source.png
```
