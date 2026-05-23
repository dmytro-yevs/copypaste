#!/usr/bin/env python3
"""
gen-icons.py — Placeholder icon generator for CopyPaste.

Generates tray icons and macOS iconset PNGs using PIL.
These are PLACEHOLDERS until a designer provides final assets.

Usage:
    python3 scripts/gen-icons.py

Outputs:
    crates/copypaste-ui/assets/tray-icon-16.png
    crates/copypaste-ui/assets/tray-icon-32.png
    crates/copypaste-ui/assets/tray-icon-active.png
    crates/copypaste-ui/assets/tray-icon-idle.png
    crates/copypaste-ui/assets/AppIcon.iconset/icon_{16,32,128,256,512}x{...}.png
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

try:
    from PIL import Image, ImageDraw, ImageFont
except ImportError:
    sys.stderr.write("ERROR: Pillow (PIL) is required. Install with: pip install Pillow\n")
    sys.exit(1)


REPO_ROOT = Path(__file__).resolve().parent.parent
ASSETS_DIR = REPO_ROOT / "crates" / "copypaste-ui" / "assets"
ICONSET_DIR = ASSETS_DIR / "AppIcon.iconset"

# Colors
MONOCHROME = (40, 40, 40, 255)     # near-black for menubar
ACTIVE_ACCENT = (50, 140, 220, 255)  # blue accent — sync active
IDLE_GRAY = (140, 140, 140, 255)   # gray — idle
BG_TRANSPARENT = (0, 0, 0, 0)


def _load_font(size: int) -> ImageFont.FreeTypeFont | ImageFont.ImageFont:
    """Try common system fonts, fall back to default bitmap font."""
    candidates = [
        "/System/Library/Fonts/Helvetica.ttc",
        "/System/Library/Fonts/SFNS.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
        "/Library/Fonts/Arial.ttf",
    ]
    for path in candidates:
        if os.path.exists(path):
            try:
                return ImageFont.truetype(path, size)
            except OSError:
                continue
    return ImageFont.load_default()


def _draw_clipboard_glyph(size: int, fg: tuple[int, int, int, int]) -> Image.Image:
    """Draw a simple clipboard glyph with a centered 'C' on transparent bg."""
    img = Image.new("RGBA", (size, size), BG_TRANSPARENT)
    draw = ImageDraw.Draw(img)

    # Clipboard body — rounded rectangle
    pad = max(1, size // 8)
    body_top = pad + max(1, size // 10)
    draw.rounded_rectangle(
        [(pad, body_top), (size - pad, size - pad)],
        radius=max(1, size // 10),
        outline=fg,
        width=max(1, size // 16),
    )

    # Clip head at top
    clip_w = size // 2
    clip_h = max(2, size // 8)
    clip_x = (size - clip_w) // 2
    draw.rounded_rectangle(
        [(clip_x, pad), (clip_x + clip_w, pad + clip_h)],
        radius=max(1, size // 20),
        fill=fg,
    )

    # Centered "C"
    font = _load_font(max(6, int(size * 0.45)))
    text = "C"
    try:
        bbox = draw.textbbox((0, 0), text, font=font)
        tw, th = bbox[2] - bbox[0], bbox[3] - bbox[1]
        tx = (size - tw) // 2 - bbox[0]
        ty = body_top + ((size - pad) - body_top - th) // 2 - bbox[1]
        draw.text((tx, ty), text, font=font, fill=fg)
    except Exception:
        # Fallback for default bitmap font — best-effort
        draw.text((size // 3, size // 3), text, fill=fg)

    return img


def write_png(img: Image.Image, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    img.save(path, format="PNG", optimize=True)
    print(f"  wrote {path.relative_to(REPO_ROOT)} ({img.size[0]}x{img.size[1]})")


def main() -> int:
    ASSETS_DIR.mkdir(parents=True, exist_ok=True)
    ICONSET_DIR.mkdir(parents=True, exist_ok=True)

    print("Generating tray icons...")
    write_png(_draw_clipboard_glyph(16, MONOCHROME), ASSETS_DIR / "tray-icon-16.png")
    write_png(_draw_clipboard_glyph(32, MONOCHROME), ASSETS_DIR / "tray-icon-32.png")
    write_png(_draw_clipboard_glyph(32, ACTIVE_ACCENT), ASSETS_DIR / "tray-icon-active.png")
    write_png(_draw_clipboard_glyph(32, IDLE_GRAY), ASSETS_DIR / "tray-icon-idle.png")

    print("Generating macOS iconset (AppIcon.iconset)...")
    for s in (16, 32, 128, 256, 512):
        write_png(_draw_clipboard_glyph(s, MONOCHROME), ICONSET_DIR / f"icon_{s}x{s}.png")

    print("Done. Placeholder icons generated.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
