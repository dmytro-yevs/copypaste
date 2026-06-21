# CopyPaste UI Assets

Icon assets for the CopyPaste desktop UI (tray menubar + macOS app icon).

> **Status: PLACEHOLDER.** The PNGs in this directory are programmatically
> generated stand-ins until a designer ships final artwork. Do not ship a
> 1.0 release without replacing them.

## Files

| File | Size | Purpose |
|------|------|---------|
| `tray-icon-16.png` | 16x16 | Menubar tray icon (standard DPI) |
| `tray-icon-32.png` | 32x32 | Menubar tray icon (retina / @2x) |
| `tray-icon-active.png` | 32x32 | Tray glyph when sync is active (blue accent) |
| `tray-icon-idle.png` | 32x32 | Tray glyph when idle (gray) |
| `AppIcon.iconset/icon_16x16.png` | 16×16 | macOS app icon source |
| `AppIcon.iconset/icon_16x16@2x.png` | 32×32 | macOS app icon @2x (retina 16pt) |
| `AppIcon.iconset/icon_32x32.png` | 32×32 | macOS app icon source |
| `AppIcon.iconset/icon_32x32@2x.png` | 64×64 | macOS app icon @2x (retina 32pt) |
| `AppIcon.iconset/icon_128x128.png` | 128×128 | macOS app icon source |
| `AppIcon.iconset/icon_128x128@2x.png` | 256×256 | macOS app icon @2x (retina 128pt) |
| `AppIcon.iconset/icon_256x256.png` | 256×256 | macOS app icon source |
| `AppIcon.iconset/icon_256x256@2x.png` | 512×512 | macOS app icon @2x (retina 256pt) |
| `AppIcon.iconset/icon_512x512.png` | 512×512 | macOS app icon source |
| `AppIcon.iconset/icon_512x512@2x.png` | 1024×1024 | macOS app icon @2x (retina 512pt) |

The iconset follows Apple's required sizes exactly: 16, 32, 128, 256, 512 (each with @2x).
Non-standard sizes (e.g. 64×64 as a standalone file) are NOT included — `iconutil` ignores
unknown filenames but their presence is noise and triggers asset-validation warnings.

`AppIcon.iconset/` is consumed by `iconutil -c icns` to produce `AppIcon.icns`
for macOS app bundles. The `gen-icons.sh` script invokes `iconutil`
automatically when run on macOS.

## Canonical `.icns` location

`AppIcon.icns` (this directory) is the **canonical generated artifact** — it is
produced by `iconutil` from the iconset source. `crates/copypaste-ui/src-tauri/icons/icon.icns`
is a build-time consumer copy that `scripts/gen-icons.sh` writes automatically after
generating `AppIcon.icns`. The two files must always match; run `gen-icons.sh` to
regenerate both together. Never edit `src-tauri/icons/icon.icns` directly.

## Regenerating

### Full platform icons (macOS + Android + Windows) — preferred

The canonical app icon SVG is `assets/logo/copypaste.svg` (1024×1024 Big Sur squircle).
To regenerate all platform icons from it — macOS `.icns`, Android mipmap PNGs,
iOS PNG, and Windows `.ico` — run:

```bash
cargo install tauri-cli            # one-time, if not installed
tauri icon assets/logo/copypaste.svg
```

This writes every required size to `crates/copypaste-ui/src-tauri/icons/`.
After running, also regenerate the macOS iconset and tray icons:

```bash
bash scripts/gen-icons.sh
```

This is the command a future CI step would call for full icon generation:
```bash
tauri icon assets/logo/copypaste.svg && bash scripts/gen-icons.sh
```

### Tray icons + macOS icns only

Drop a vector source at `crates/copypaste-ui/assets/tray-icon.svg`, then:

```bash
bash scripts/gen-icons.sh
```

Requires ImageMagick (`brew install imagemagick`). The script rasterizes
every required size from the single SVG source and (on macOS) packages
`AppIcon.icns`, then copies it to `src-tauri/icons/icon.icns`.

### PIL placeholders (fallback)

If no SVG exists or ImageMagick is missing, `gen-icons.sh` falls back to
`scripts/gen-icons.py`, which uses Python + Pillow to draw a minimal
clipboard glyph with a centered `C`:

```bash
python3 scripts/gen-icons.py
```

Requires `pip install Pillow`.

## Licensing

These placeholder PNGs and the generator scripts are part of the CopyPaste
project and inherit the repository license (see `LICENSE` at the repo root).

Once a designer-provided SVG is committed, document its origin and license
in this file directly above the **Regenerating** section.

## Manual replacement

If you have final PNGs from a designer that should bypass regeneration,
drop them in place by filename, then commit. Do **not** delete this
README — update the **Status** banner at the top to remove the placeholder
warning instead.
