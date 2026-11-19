"""Draws the File Chameleon app icon (assets/icon-source.png, 1024x1024).

A macOS-style squircle with a colour-shifting gradient (a chameleon changes colour,
the app changes formats) and a white chameleon glyph: curled tail, body arc, head, eye.
Run: python assets/make_icon.py   then   npx tauri icon assets/icon-source.png -o src-tauri/icons
"""
import math
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

S = 4096  # supersampled canvas
OUT = Path(__file__).with_name("icon-source.png")


def squircle_mask(size: int, margin: float, n: float = 5.0) -> Image.Image:
    m = Image.new("L", (size, size), 0)
    d = ImageDraw.Draw(m)
    c = size / 2
    r = size / 2 - margin
    pts = []
    for i in range(720):
        t = 2 * math.pi * i / 720
        ct, st = math.cos(t), math.sin(t)
        x = c + r * math.copysign(abs(ct) ** (2 / n), ct)
        y = c + r * math.copysign(abs(st) ** (2 / n), st)
        pts.append((x, y))
    d.polygon(pts, fill=255)
    return m


def gradient(size: int) -> Image.Image:
    stops = [(0.0, (52, 199, 89)), (0.45, (48, 176, 199)), (1.0, (94, 92, 230))]
    small = 256
    g = Image.new("RGB", (small, small))
    px = g.load()
    for y in range(small):
        for x in range(small):
            t = (x + y) / (2 * (small - 1))
            for (t0, c0), (t1, c1) in zip(stops, stops[1:]):
                if t0 <= t <= t1:
                    k = (t - t0) / (t1 - t0)
                    px[x, y] = tuple(int(a + (b - a) * k) for a, b in zip(c0, c1))
                    break
    return g.resize((size, size), Image.BICUBIC)


def main() -> None:
    margin = S * 0.085  # Apple icon grid: artwork inset inside the canvas
    mask = squircle_mask(S, margin)
    base = Image.new("RGBA", (S, S), (0, 0, 0, 0))

    # Soft drop shadow under the tile.
    shadow = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    sm = mask.filter(ImageFilter.GaussianBlur(S * 0.02))
    shadow.paste((0, 0, 0, 90), (0, int(S * 0.012)), sm)
    base = Image.alpha_composite(base, shadow)

    tile = gradient(S).convert("RGBA")
    # Glassy top highlight.
    hl = Image.new("L", (S, S), 0)
    ImageDraw.Draw(hl).ellipse((-S * 0.3, -S * 0.75, S * 1.3, S * 0.48), fill=70)
    hl = hl.filter(ImageFilter.GaussianBlur(S * 0.03))
    tile = Image.composite(Image.new("RGBA", (S, S), (255, 255, 255, 255)), tile, hl)
    tile.putalpha(mask)
    base = Image.alpha_composite(base, tile)

    glyph = Image.new("L", (S, S), 0)
    d = ImageDraw.Draw(glyph)
    w = int(S * 0.062)

    def stamp(path):
        # Stroke by stamping discs: smooth joins at any curvature (PIL thick lines gap).
        prev = None
        for p in path:
            if prev is not None:
                dist = math.hypot(p[0] - prev[0], p[1] - prev[1])
                n = max(1, int(dist / (w * 0.15)))
                for k in range(1, n + 1):
                    x = prev[0] + (p[0] - prev[0]) * k / n
                    y = prev[1] + (p[1] - prev[1]) * k / n
                    d.ellipse((x - w / 2, y - w / 2, x + w / 2, y + w / 2), fill=255)
            prev = p

    def cubic(p0, p1, p2, p3, steps=240):
        out = []
        for i in range(steps + 1):
            t = i / steps
            u = 1 - t
            out.append((
                u**3 * p0[0] + 3 * u * u * t * p1[0] + 3 * u * t * t * p2[0] + t**3 * p3[0],
                u**3 * p0[1] + 3 * u * u * t * p1[1] + 3 * u * t * t * p2[1] + t**3 * p3[1],
            ))
        return out

    # Head (top-left), body arching right, tail curling clockwise into a spiral (bottom-right).
    head = (S * 0.31, S * 0.37)
    cx, cy, r0 = S * 0.60, S * 0.655, S * 0.17
    turns, r_end = 1.35, S * 0.034
    b = math.log(r0 / r_end) / (2 * math.pi * turns)
    body = cubic((S * 0.40, S * 0.40), (S * 0.56, S * 0.25), (S * (0.60 + 0.17 + 0.02), S * 0.40), (cx + r0, cy))
    tail = []
    for i in range(1, 801):
        th = 2 * math.pi * turns * i / 800
        r = r0 * math.exp(-b * th)
        tail.append((cx + r * math.cos(th), cy + r * math.sin(th)))
    stamp(body + tail)
    hr = S * 0.105
    d.ellipse((head[0] - hr * 1.15, head[1] - hr * 0.9, head[0] + hr * 1.05, head[1] + hr * 0.9), fill=255)
    d.polygon(
        [(head[0] - hr * 0.55, head[1] - hr * 0.55), (head[0] - hr * 0.05, head[1] - hr * 1.45), (head[0] + hr * 0.6, head[1] - hr * 0.6)],
        fill=255,
    )
    glyph_img = Image.new("RGBA", (S, S), (255, 255, 255, 0))
    glyph_img.putalpha(glyph)

    # Eye: punched hole + coloured pupil.
    eye = Image.new("L", (S, S), 0)
    er = hr * 0.42
    ex, ey = head[0] + hr * 0.1, head[1] - hr * 0.08
    ImageDraw.Draw(eye).ellipse((ex - er, ey - er, ex + er, ey + er), fill=255)
    glyph_img.putalpha(Image.composite(Image.new("L", (S, S), 0), glyph_img.getchannel("A"), eye))
    pupil = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    pr = er * 0.55
    ImageDraw.Draw(pupil).ellipse((ex - pr * 0.6, ey - pr, ex + pr * 1.4, ey + pr), fill=(255, 255, 255, 255))

    gl_shadow = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    gl_shadow.paste((0, 40, 60, 70), (0, int(S * 0.008)), glyph_img.getchannel("A").filter(ImageFilter.GaussianBlur(S * 0.008)))
    base = Image.alpha_composite(base, gl_shadow)
    base = Image.alpha_composite(base, glyph_img)
    base = Image.alpha_composite(base, pupil)

    base.resize((1024, 1024), Image.LANCZOS).save(OUT)
    print("wrote", OUT)


if __name__ == "__main__":
    main()
