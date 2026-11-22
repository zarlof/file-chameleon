"""Builds the app icon from assets/icon-artwork.png (needs Pillow, numpy and scipy).

    python assets/make_icon.py cutout   # artwork -> assets/icon-source.png (no tile, transparent, 1024 px)
    npx tauri icon assets/icon-source.png -o src-tauri/icons   (then delete icons/android and icons/ios)
    python assets/make_icon.py ico      # sharper 16-48 px frames -> src-tauri/icons/icon.ico + 32x32.png
"""
import io
import struct
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter
from scipy import ndimage as ndi

ROOT = Path(__file__).resolve().parent.parent
ART = ROOT / 'assets' / 'icon-artwork.png'
SRC = ROOT / 'assets' / 'icon-source.png'
ICONS = ROOT / 'src-tauri' / 'icons'


def cutout():
    """Drops the dark rounded tile: keeps the chameleon, the folder and the paper."""
    a = np.asarray(Image.open(ART).convert('RGB')).astype(np.float32)
    sat = a.max(2) - a.min(2)
    lum = a @ np.array([0.299, 0.587, 0.114], np.float32)
    # Tile pixels are dark and grey. Regions of them count as background when they touch the
    # border or are large (the gap between the tail and the folder); small ones (the pupil) stay.
    lab, n = ndi.label((sat < 32) & (lum < 120))
    border = set(np.unique(np.concatenate([lab[0], lab[-1], lab[:, 0], lab[:, -1]]))) - {0}
    areas = ndi.sum(np.ones_like(lab), lab, range(1, n + 1))
    bg_ids = [i for i in range(1, n + 1) if i in border or areas[i - 1] > 1000]
    bg = np.isin(lab, bg_ids)
    # Grow into the very dark shading that borders it (dark rims along the gap and the edges).
    bg = ndi.binary_propagation(bg, mask=bg | (lum < 75))
    subject = ~bg
    # Largest blob only (drops specks). No hole filling: the gap behind the tail is a hole.
    blobs, m = ndi.label(subject)
    sizes = ndi.sum(subject, blobs, range(1, m + 1))
    subject = blobs == 1 + int(np.argmax(sizes))
    # Edge pixels are mixed with the dark tile: give the rim the colour of the nearest inner pixel.
    inner = ndi.binary_erosion(subject, iterations=3)
    _, (iy, ix) = ndi.distance_transform_edt(~inner, return_indices=True)
    rgb = a[iy, ix]
    alpha = ndi.gaussian_filter(ndi.binary_erosion(subject, iterations=1).astype(np.float32), 0.9)
    alpha = np.clip((alpha - 0.1) / 0.8, 0, 1)
    im = Image.fromarray(np.dstack([rgb, alpha * 255]).astype(np.uint8), 'RGBA')
    im = im.crop(im.getbbox())
    # Fill the canvas: the longer side is 1024 - 2 * 20 px.
    side, margin = 1024, 20
    scale = (side - 2 * margin) / max(im.size)
    im = im.resize((round(im.width * scale), round(im.height * scale)), Image.LANCZOS)
    canvas = Image.new('RGBA', (side, side), (0, 0, 0, 0))
    canvas.alpha_composite(im, ((side - im.width) // 2, (side - im.height) // 2))
    canvas.save(SRC, optimize=True)


def frame(big, s):
    """Small sizes get a light sharpen so the chameleon stays readable."""
    f = big.resize((s, s), Image.LANCZOS)
    if s <= 48:
        f = f.filter(ImageFilter.UnsharpMask(radius=0.6, percent=60, threshold=1))
    return f


def ico():
    big = Image.open(SRC).convert('RGBA')
    sizes = [16, 24, 32, 48, 64, 256]
    blobs = []
    for s in sizes:
        b = io.BytesIO()
        frame(big, s).save(b, 'PNG', optimize=True)
        blobs.append(b.getvalue())
    head = struct.pack('<HHH', 0, 1, len(sizes))
    off = 6 + 16 * len(sizes)
    entries = b''
    for s, b in zip(sizes, blobs):
        entries += struct.pack('<BBBBHHII', s % 256, s % 256, 0, 0, 1, 32, len(b), off)
        off += len(b)
    (ICONS / 'icon.ico').write_bytes(head + entries + b''.join(blobs))
    frame(big, 32).save(ICONS / '32x32.png', optimize=True)


if __name__ == '__main__':
    {'cutout': cutout, 'ico': ico}[sys.argv[1]]()
