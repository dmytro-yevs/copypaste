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
| `AppIcon.iconset/icon_16x16.png` | 16x16 | macOS app icon source |
| `AppIcon.iconset/icon_32x32.png` | 32x32 | macOS app icon source |
| `AppIcon.iconset/icon_128x128.png` | 128x128 | macOS app icon source |
| `AppIcon.iconset/icon_256x256.png` | 256x256 | macOS app icon source |
| `AppIcon.iconset/icon_512x512.png` | 512x512 | macOS app icon source |

`AppIcon.iconset/` is consumed by `iconutil -c icns` to produce `AppIcon.icns`
for macOS app bundles. The `gen-icons.sh` script invokes `iconutil`
automatically when run on macOS.

## Regenerating

Two paths:

### 1. From a designer-supplied SVG (preferred for production)

Drop a vector source at `crates/copypaste-ui/assets/tray-icon.svg`, then:

```bash
bash scripts/gen-icons.sh
```

Requires ImageMagick (`brew install imagemagick`). The script rasterizes
every required size from the single SVG source and (on macOS) packages
`AppIcon.icns`.

### 2. PIL placeholders (current default)

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
