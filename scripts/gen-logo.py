#!/usr/bin/env python3
"""Generate the WireCraft app icon: an engraved octopus gripping a wire, on a periwinkle tile.

Writes assets/app-icon/wirecraft.svg (full detail), wirecraft-small.svg (no hatching, for 16-32 px) and the
hicolor PNG set under assets/app-icon/hicolor/. Tauri icons are then made with `npx tauri icon`.

    python3 scripts/gen-logo.py
"""
import math
import os
import subprocess

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "assets", "app-icon")
APP_ID = "dev.wirecraft.app"

INK = "#14163a"  # deep indigo engraving ink
TILE_TOP = "#c9ccff"
TILE_BOTTOM = "#8b93ff"  # accent/default from design/generator/lib.py
PAPER = "#eef0ff"  # carved highlights

# ---- the octopus: mantle (head), eyes with horizontal pupils, eight tapered arms gripping a wire
MANTLE = (
    "M512 150 C664 150 736 268 726 392 C718 478 664 540 606 566 "
    "L418 566 C360 540 306 478 298 392 C288 268 360 150 512 150 Z"
)
EYES = [(440, 492), (584, 492)]
WIRE_Y = 742


def catmull(points, steps=14):
    """Smooth curve through points (Catmull-Rom), as a list of samples."""
    pts = [points[0]] + points + [points[-1]]
    out = []
    for i in range(1, len(pts) - 2):
        p0, p1, p2, p3 = pts[i - 1], pts[i], pts[i + 1], pts[i + 2]
        for k in range(steps):
            t = k / steps
            t2, t3 = t * t, t * t * t
            out.append(tuple(
                0.5 * ((2 * p1[j]) + (-p0[j] + p2[j]) * t + (2 * p0[j] - 5 * p1[j] + 4 * p2[j] - p3[j]) * t2
                       + (-p0[j] + 3 * p1[j] - 3 * p2[j] + p3[j]) * t3)
                for j in range(2)))
    out.append(points[-1])
    return out


def arm(points, w0, w1):
    """A tapered arm: (outline path, sucker positions)."""
    c = catmull(points)
    left, right, suckers = [], [], []
    n = len(c)
    for i, (x, y) in enumerate(c):
        a = c[max(0, i - 1)]
        b = c[min(n - 1, i + 1)]
        dx, dy = b[0] - a[0], b[1] - a[1]
        ln = math.hypot(dx, dy) or 1
        nx, ny = -dy / ln, dx / ln
        t = i / (n - 1)
        w = (w0 + (w1 - w0) * t) / 2
        left.append((x + nx * w, y + ny * w))
        right.append((x - nx * w, y - ny * w))
        if 3 <= i < n - 4 and i % 4 == 0:
            suckers.append((x - nx * w * 0.45, y - ny * w * 0.45, max(4.0, w * 0.34)))
    poly = left + right[::-1]
    d = "M" + " L".join(f"{x:.1f} {y:.1f}" for x, y in poly) + " Z"
    return d, suckers


# centrelines, left half; the right half is mirrored. The two inner pairs grip the wire.
ARMS_LEFT = [
    ([(424, 548), (350, 590), (262, 616), (196, 596), (178, 548), (214, 528), (238, 554)], 54, 10),
    ([(446, 560), (396, 650), (318, 718), (260, 792), (282, 846), (326, 836), (322, 806)], 52, 10),
    ([(466, 568), (430, 650), (392, WIRE_Y - 36), (378, WIRE_Y + 20), (404, WIRE_Y + 48), (430, WIRE_Y + 18)], 50, 12),
    ([(494, 572), (482, 670), (470, WIRE_Y - 10), (462, 820), (440, 868), (404, 878), (392, 850)], 44, 10),
]


def mirror(points):
    return [(1024 - x, y) for x, y in points]


def hatch_lines(y0, y1, step, bulge, x0=280, x1=744):
    out = []
    y = y0
    while y <= y1:
        out.append(f'<path d="M{x0} {y:.1f} Q512 {y + bulge:.1f} {x1} {y:.1f}"/>')
        y += step
    return "\n      ".join(out)


def svg(detail=True):
    arms = []
    for pts, w0, w1 in ARMS_LEFT:
        arms.append(arm(pts, w0, w1))
        arms.append(arm(mirror(pts), w0, w1))
    # back arms first, the gripping pair last so they sit over the wire
    back, front = arms[:4], arms[4:]
    wire = (
        f'<path d="M48 {WIRE_Y + 26} C320 {WIRE_Y - 8} 704 {WIRE_Y - 8} 976 {WIRE_Y + 26}" fill="none" stroke="{INK}" stroke-width="{24 if detail else 34}" stroke-linecap="round"/>'
        + (f'<path d="M80 {WIRE_Y + 20} C330 {WIRE_Y - 10} 694 {WIRE_Y - 10} 944 {WIRE_Y + 20}" fill="none" stroke="{PAPER}" stroke-width="5" stroke-linecap="round" opacity=".55"/>' if detail else "")
    )

    def draw(group):
        out = []
        for d, suckers in group:
            out.append(f'<path d="{d}" fill="{INK}" stroke="{INK}" stroke-width="4" stroke-linejoin="round"/>')
            if detail:
                out.extend(
                    f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{r:.1f}" fill="none" stroke="{PAPER}" stroke-width="4.5"/>'
                    for x, y, r in suckers
                )
        return "".join(out)

    eyes = []
    for (x, y) in EYES:
        eyes.append(
            f'<circle cx="{x}" cy="{y}" r="{46 if detail else 50}" fill="{PAPER}" stroke="{INK}" stroke-width="{10 if detail else 16}"/>'
            + (f'<circle cx="{x}" cy="{y}" r="34" fill="none" stroke="{INK}" stroke-width="4"/>' if detail else "")
            + f'<rect x="{x - 26}" y="{y - 10}" width="52" height="20" rx="10" fill="{INK}"/>'
            + f'<circle cx="{x + 12}" cy="{y - 18}" r="{8 if detail else 10}" fill="{PAPER}" stroke="{INK}" stroke-width="3"/>'
        )
    if detail:
        mantle = f"""
  <path d="{MANTLE}" fill="{PAPER}"/>
  <g clip-path="url(#mantle)" fill="none" stroke="{INK}" stroke-linecap="round">
    <g stroke-width="7">
      {hatch_lines(150, 580, 19, -30)}
    </g>
    <ellipse cx="452" cy="270" rx="70" ry="48" fill="{PAPER}" stroke="none" transform="rotate(-24 452 270)"/>
    <g stroke-width="5" opacity=".9">
      <path d="M404 252 Q440 222 492 228"/><path d="M398 290 Q430 262 470 262"/>
    </g>
  </g>
  <path d="{MANTLE}" fill="none" stroke="{INK}" stroke-width="14" stroke-linejoin="round"/>"""
    else:
        mantle = f'<path d="{MANTLE}" fill="{INK}"/>'
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">
  <defs>
    <linearGradient id="tile" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="{TILE_TOP}"/><stop offset="1" stop-color="{TILE_BOTTOM}"/>
    </linearGradient>
    <clipPath id="mantle"><path d="{MANTLE}"/></clipPath>
    <clipPath id="tileclip"><rect width="1024" height="1024" rx="228"/></clipPath>
  </defs>
  <rect width="1024" height="1024" rx="228" fill="url(#tile)"/>
  <rect x="10" y="10" width="1004" height="1004" rx="220" fill="none" stroke="#ffffff" stroke-opacity=".35" stroke-width="6"/>
  <g clip-path="url(#tileclip)">
  <g transform="translate(512 {512 if detail else 500}) scale({1.0 if detail else 1.18}) translate(-512 -512)">
  {draw(back)}
  {wire}
  {mantle}
  {draw(front)}
  {"".join(eyes)}
  </g>
  </g>
</svg>
"""


def main():
    os.makedirs(OUT, exist_ok=True)
    full = os.path.join(OUT, "wirecraft.svg")
    small = os.path.join(OUT, "wirecraft-small.svg")
    open(full, "w").write(svg(True))
    open(small, "w").write(svg(False))
    # the splash screen (public/splash.html) loads before the app bundle, so it gets its own copy
    open(os.path.join(ROOT, "public", "icon.svg"), "w").write(svg(True))
    for size in (16, 24, 32, 48, 64, 128, 256, 512, 1024):
        d = os.path.join(OUT, "hicolor", f"{size}x{size}", "apps")
        os.makedirs(d, exist_ok=True)
        src = small if size <= 32 else full
        subprocess.run(["rsvg-convert", "-w", str(size), "-h", str(size), src, "-o", os.path.join(d, f"{APP_ID}.png")], check=True)
    print("wrote", full, small, "and hicolor PNGs")


if __name__ == "__main__":
    main()
