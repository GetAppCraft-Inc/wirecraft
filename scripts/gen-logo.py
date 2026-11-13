#!/usr/bin/env python3
"""Generate the WireCraft app icon: an engraved owl perched on a wire, on a periwinkle tile.

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

# Owl silhouette (head with ear tufts and body), 1024 x 1024 canvas.
BODY = (
    "M512 300 C452 292 392 262 334 214 C344 268 336 306 322 346 "
    "C284 424 274 520 294 602 C322 700 410 758 512 764 "
    "C614 758 702 700 730 602 C750 520 740 424 702 346 "
    "C688 306 680 268 690 214 C632 262 572 292 512 300 Z"
)
# Facial disc: two lobes around the eyes, meeting above the beak.
DISC = (
    "M512 330 C472 300 382 300 352 360 C326 414 340 486 400 512 "
    "C446 532 490 520 512 552 C534 520 578 532 624 512 "
    "C684 486 698 414 672 360 C642 300 552 300 512 330 Z"
)
WING_L = "M300 470 C268 560 286 662 352 724 C376 662 384 584 372 506 C352 480 324 468 300 470 Z"
WING_R = "M724 470 C756 560 738 662 672 724 C648 662 640 584 652 506 C672 480 700 468 724 470 Z"
EYES = [(436, 416), (588, 416)]


def hatch_lines(y0, y1, step, bulge, x0=230, x1=794):
    out = []
    y = y0
    while y <= y1:
        out.append(f'<path d="M{x0} {y:.1f} Q512 {y + bulge:.1f} {x1} {y:.1f}"/>')
        y += step
    return "\n      ".join(out)


def diag_lines(x0, x1, y0, y1, step, slope):
    out = []
    x = x0
    while x <= x1:
        out.append(f'<path d="M{x:.1f} {y0} L{x + (y1 - y0) * slope:.1f} {y1}"/>')
        x += step
    return "\n      ".join(out)


def scallops(cx, y0, rows, cols, w, h, lift):
    """Small U-shaped feather marks across the belly."""
    out = []
    for r in range(rows):
        y = y0 + r * h
        n = cols - (1 if r % 2 else 0)
        width = n * w
        for c in range(n):
            x = cx - width / 2 + c * w
            out.append(f'<path d="M{x:.1f} {y:.1f} Q{x + w / 2:.1f} {y + lift:.1f} {x + w:.1f} {y:.1f}"/>')
    return "\n      ".join(out)


def svg(detail=True):
    eyes = []
    for (x, y) in EYES:
        if detail:
            rings = "".join(
                f'<circle cx="{x}" cy="{y}" r="{r}" fill="none" stroke="{INK}" stroke-width="{w}"/>'
                for r, w in [(86, 9), (72, 5), (62, 5)]
            )
        else:
            rings = f'<circle cx="{x}" cy="{y}" r="80" fill="none" stroke="{INK}" stroke-width="16"/>'
        eyes.append(
            f'<circle cx="{x}" cy="{y}" r="86" fill="{PAPER}"/>{rings}'
            f'<circle cx="{x}" cy="{y}" r="{40 if detail else 46}" fill="{INK}"/>'
            f'<circle cx="{x + 13}" cy="{y - 14}" r="{12 if detail else 15}" fill="{PAPER}"/>'
        )
    beak = f'<path d="M512 456 L536 486 L512 540 L488 486 Z" fill="{INK}"/><path d="M512 466 L524 486 L512 512 Z" fill="{PAPER}" opacity=".55"/>'
    wire = (
        f'<path d="M64 772 C320 812 704 812 960 760" fill="none" stroke="{INK}" stroke-width="{26 if detail else 34}" stroke-linecap="round"/>'
        + (f'<path d="M96 768 C330 802 700 802 930 758" fill="none" stroke="{PAPER}" stroke-width="5" stroke-linecap="round" opacity=".55"/>' if detail else "")
    )
    claws = "".join(
        f'<path d="M{x - 26} 762 q6 34 14 40 M{x} 766 q2 36 4 44 M{x + 26} 762 q-4 34 -12 40" fill="none" stroke="{INK}" stroke-width="{12 if detail else 16}" stroke-linecap="round"/>'
        for x in (466, 558)
    )
    if detail:
        body = f"""
  <path d="{BODY}" fill="{PAPER}"/>
  <g clip-path="url(#body)" fill="none" stroke="{INK}" stroke-linecap="round">
    <g stroke-width="7">
      {hatch_lines(220, 780, 19, 34)}
    </g>
    <g stroke="{PAPER}" stroke-width="9">
      {scallops(512, 588, 5, 6, 44, 30, 26)}
    </g>
  </g>
  <g clip-path="url(#wings)">
    <path d="{WING_L}" fill="{INK}"/><path d="{WING_R}" fill="{INK}"/>
    <g fill="none" stroke="{PAPER}" stroke-width="6" stroke-linecap="round" opacity=".85">
      {diag_lines(230, 420, 460, 740, 20, 0.28)}
      {diag_lines(600, 800, 460, 740, 20, -0.28)}
    </g>
  </g>
  <path d="{DISC}" fill="{PAPER}"/>
  <g clip-path="url(#disc)" fill="none" stroke="{INK}" stroke-width="5" stroke-linecap="round" opacity=".9">
    {"".join(f'<circle cx="{x}" cy="{y}" r="{r}"/>' for (x, y) in EYES for r in range(104, 170, 15))}
  </g>
  <path d="{DISC}" fill="none" stroke="{INK}" stroke-width="10"/>
  <path d="{BODY}" fill="none" stroke="{INK}" stroke-width="14" stroke-linejoin="round"/>"""
    else:
        body = f"""
  <path d="{BODY}" fill="{INK}"/>
  <path d="{DISC}" fill="{PAPER}"/>"""
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">
  <defs>
    <linearGradient id="tile" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="{TILE_TOP}"/><stop offset="1" stop-color="{TILE_BOTTOM}"/>
    </linearGradient>
    <clipPath id="body"><path d="{BODY}"/></clipPath>
    <clipPath id="disc"><path d="{DISC}"/></clipPath>
    <clipPath id="wings"><path d="{WING_L}"/><path d="{WING_R}"/></clipPath>
  </defs>
  <rect width="1024" height="1024" rx="228" fill="url(#tile)"/>
  <rect x="10" y="10" width="1004" height="1004" rx="220" fill="none" stroke="#ffffff" stroke-opacity=".35" stroke-width="6"/>
  <g transform="translate(512 {500 if detail else 470}) scale({1.14 if detail else 1.42}) translate(-512 -500)">
  {wire}
  {body}
  {"".join(eyes)}
  {beak}
  {claws}
  </g>
</svg>
"""


def main():
    os.makedirs(OUT, exist_ok=True)
    full = os.path.join(OUT, "wirecraft.svg")
    small = os.path.join(OUT, "wirecraft-small.svg")
    open(full, "w").write(svg(True))
    open(small, "w").write(svg(False))
    for size in (16, 24, 32, 48, 64, 128, 256, 512, 1024):
        d = os.path.join(OUT, "hicolor", f"{size}x{size}", "apps")
        os.makedirs(d, exist_ok=True)
        src = small if size <= 32 else full
        subprocess.run(["rsvg-convert", "-w", str(size), "-h", str(size), src, "-o", os.path.join(d, f"{APP_ID}.png")], check=True)
    print("wrote", full, small, "and hicolor PNGs")


if __name__ == "__main__":
    main()
