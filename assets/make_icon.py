"""Builds the app icon from assets/icon-artwork.png (needs Pillow).

    python assets/make_icon.py tile   # artwork -> assets/icon-source.png (transparent corners, 1024 px)
    npx tauri icon assets/icon-source.png -o src-tauri/icons   (then delete icons/android and icons/ios)
    python assets/make_icon.py ico    # sharper 16-48 px frames -> src-tauri/icons/icon.ico + 32x32.png
"""
import io
import struct
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

ROOT = Path(__file__).resolve().parent.parent
ART = ROOT / 'assets' / 'icon-artwork.png'
SRC = ROOT / 'assets' / 'icon-source.png'
ICONS = ROOT / 'src-tauri' / 'icons'


def tile():
    im = Image.open(ART).convert('RGB')
    # The rounded tile spans 128..896 in the 1024 px artwork, corner radius ~168.
    left, top, side, radius, inset, k = 128, 128, 768, 168, 2, 4
    t = im.crop((left, top, left + side, top + side)).convert('RGBA')
    m = Image.new('L', (side * k, side * k), 0)
    ImageDraw.Draw(m).rounded_rectangle(
        (inset * k, inset * k, (side - inset) * k - 1, (side - inset) * k - 1),
        radius=(radius - inset) * k, fill=255)
    t.putalpha(m.resize((side, side), Image.LANCZOS))
    canvas_side, size = 1024, 944
    t = t.resize((size, size), Image.LANCZOS)
    off = (canvas_side - size) // 2
    shadow = Image.new('RGBA', (canvas_side, canvas_side), (0, 0, 0, 0))
    shadow.paste((0, 0, 0, 255), (off, off + 8), t.split()[3].point(lambda a: a * 0.35))
    canvas = shadow.filter(ImageFilter.GaussianBlur(12))
    canvas.alpha_composite(t, (off, off))
    canvas.save(SRC, optimize=True)


def frame(big, plate, s):
    """Small sizes: the tile fills the frame (no shadow margin) and gets a light sharpen."""
    if s <= 48:
        pad = 0 if s <= 24 else max(1, s // 32)
        t = plate.resize((s - 2 * pad, s - 2 * pad), Image.LANCZOS)
        t = t.filter(ImageFilter.UnsharpMask(radius=0.6, percent=60, threshold=1))
        f = Image.new('RGBA', (s, s), (0, 0, 0, 0))
        f.alpha_composite(t, (pad, pad))
        return f
    return big.resize((s, s), Image.LANCZOS)


def ico():
    big = Image.open(SRC).convert('RGBA')
    plate = big.crop((40, 40, 984, 984))
    sizes = [16, 24, 32, 48, 64, 256]
    blobs = []
    for s in sizes:
        b = io.BytesIO()
        frame(big, plate, s).save(b, 'PNG', optimize=True)
        blobs.append(b.getvalue())
    head = struct.pack('<HHH', 0, 1, len(sizes))
    off = 6 + 16 * len(sizes)
    entries = b''
    for s, b in zip(sizes, blobs):
        entries += struct.pack('<BBBBHHII', s % 256, s % 256, 0, 0, 1, 32, len(b), off)
        off += len(b)
    (ICONS / 'icon.ico').write_bytes(head + entries + b''.join(blobs))
    frame(big, plate, 32).save(ICONS / '32x32.png', optimize=True)


if __name__ == '__main__':
    {'tile': tile, 'ico': ico}[sys.argv[1]]()
