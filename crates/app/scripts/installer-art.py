#!/usr/bin/env python3
"""The installers' pictures, in the illuminated look: the Windows installer's sidebar and header
(24-bit BMPs, the sizes NSIS's Modern UI wants) and the Mac disk image's background (a TIFF with
the 1x and 2x pictures, so it's sharp on Retina screens).

Everything is drawn on whole pixels: the G is icons/source/icon.svg's own cells, the words are
the app's pixel fonts (crates/ui/fonts) with smoothing off. Windows stretches the pictures by
125% to 200% on bigger displays, so the sidebar's G uses 4px cells, which land on whole pixels
at every one of those steps.

Run it from anywhere after changing the icon or the colours: python3 crates/app/scripts/installer-art.py
Needs Pillow, and tiffutil (macOS) for the disk image's TIFF.
"""
import re
import subprocess
import tempfile
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

APP = Path(__file__).resolve().parent.parent
FONTS = APP.parent / "ui" / "fonts"
OUT = APP / "icons" / "installer"

# brand.css
VELLUM = (0xEF, 0xE4, 0xCC)
IRON_GALL = (0x1D, 0x17, 0x12)
VERMILION = (0xB3, 0x30, 0x1C)

# the G's cells on the icon's 32-cell grid
CELLS = [(int(x), int(y)) for x, y in re.findall(r'<rect x="(\d+)" y="(\d+)" width="1" height="1"/>', (APP / "icons/source/icon.svg").read_text())]
GX0, GY0 = min(x for x, _ in CELLS), min(y for _, y in CELLS)
GW, GH = max(x for x, _ in CELLS) - GX0 + 1, max(y for _, y in CELLS) - GY0 + 1


def g(draw, left, top, cell, colour):
    """The G with its top-left ink at (left, top), `cell` pixels to a cell."""
    for x, y in CELLS:
        px, py = left + (x - GX0) * cell, top + (y - GY0) * cell
        draw.rectangle([px, py, px + cell - 1, py + cell - 1], fill=colour)


def font(name, size):
    return ImageFont.truetype(str(FONTS / name), size)


def words(draw, centre_x, top, text, face, colour):
    """`text` centred on `centre_x`, its ink starting at `top`, unsmoothed."""
    l, t, r, _ = draw.textbbox((0, 0), text, font=face)
    draw.text((centre_x - (r - l) // 2 - l, top - t), text, font=face, fill=colour)


def double_rule(draw, x0, x1, y, colour, line):
    """The page heads' rule: two lines `line` thick with a gap the same."""
    draw.rectangle([x0, y, x1, y + line - 1], fill=colour)
    draw.rectangle([x0, y + 2 * line, x1, y + 3 * line - 1], fill=colour)


def canvas(w, h, colour):
    im = Image.new("RGB", (w, h), colour)
    d = ImageDraw.Draw(im)
    d.fontmode = "1"  # whole pixels only
    return im, d


def sidebar():
    """164x314, beside the welcome and finish pages: a vermilion leaf with the G in vellum."""
    im, d = canvas(164, 314, VERMILION)
    cell = 4
    g(d, (164 - GW * cell) // 2, 72, cell, VELLUM)
    double_rule(d, 24, 139, 216, VELLUM, 2)
    words(d, 82, 236, "gryphon.", font("jacquard-24-latin-400-normal.woff2", 48), VELLUM)
    return im


def header():
    """150x57, at the right of the header on every other page: the G as an initial in its box."""
    im, d = canvas(150, 57, VELLUM)
    cell = 2
    box = GW * cell + 8
    d.rectangle([150 - box, 0, 149, 56], fill=VERMILION)
    g(d, 150 - box + 4, (57 - GH * cell) // 2, cell, VELLUM)
    return im


# The disk image's window: Finder puts the icons' centres at these points, 1x. Newer Finders
# ignore the size a disk image asks for and open it at the user's usual window size, so the
# vellum runs on past the art: a bigger window shows more page, never white.
DMG_W, DMG_H = 600, 380
DMG_PAGE_W, DMG_PAGE_H = 1600, 1000
DMG_APP, DMG_APPLICATIONS, DMG_ICON_Y = 150, 450, 200


def dmg(scale):
    """The disk image's background at 1x or 2x: the wordmark, an arrow between the two icons,
    and what to do, on vellum."""
    s = scale
    im, d = canvas(DMG_PAGE_W * s, DMG_PAGE_H * s, VELLUM)
    # the wordmark as the app sets it: the g in a vermilion box, the rest in ink
    jac = font("jacquard-24-latin-400-normal.woff2", 48 * s)
    gl, _, gr, _ = d.textbbox((0, 0), "g", font=jac, anchor="ls")
    rl, _, rr, _ = d.textbbox((0, 0), "ryphon.", font=jac, anchor="ls")
    _, top, _, bottom = d.textbbox((0, 0), "gryphon.", font=jac, anchor="ls")  # ink above and below the baseline
    pad = 4 * s  # .wordmark .initial: 4px round the g, then 4px to the rest
    box = (gr - gl) + 2 * pad
    x = (DMG_W * s - (box + pad + rr - rl)) // 2
    base = 88 * s
    d.rectangle([x, base + top - pad, x + box - 1, base + bottom + pad - 1], fill=VERMILION)
    d.text((x + pad - gl, base), "g", font=jac, fill=VELLUM, anchor="ls")
    d.text((x + box + pad - rl, base), "ryphon.", font=jac, fill=IRON_GALL, anchor="ls")
    double_rule(d, 48 * s, (DMG_W - 48) * s - 1, 120 * s, IRON_GALL, 2 * s)
    # the arrow: a shaft two cells thick and a stepped head, 4px cells
    c = 4 * s
    cy = DMG_ICON_Y * s
    x0, x1 = (DMG_APP + 84) * s, (DMG_APPLICATIONS - 84) * s
    head = 5
    d.rectangle([x0, cy - c, x1 - head * c - 1, cy + c - 1], fill=VERMILION)
    for i in range(head):
        hx = x1 - (head - i) * c
        reach = (head - i) * c
        d.rectangle([hx, cy - reach, hx + c - 1, cy + reach - 1], fill=VERMILION)
    words(d, DMG_W * s // 2, 316 * s, "Drag Gryphon into Applications", font("jersey-15-latin-400-normal.woff2", 30 * s), IRON_GALL)
    return im


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    sidebar().save(OUT / "sidebar.bmp")
    header().save(OUT / "header.bmp")
    with tempfile.TemporaryDirectory() as tmp:  # only the TIFF made of them is kept
        one, two = Path(tmp, "dmg-background.png"), Path(tmp, "dmg-background@2x.png")
        dmg(1).save(one)
        dmg(2).save(two)
        subprocess.run(["tiffutil", "-cathidpicheck", str(one), str(two), "-out", str(OUT / "dmg-background.tiff")], check=True, capture_output=True)
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()
