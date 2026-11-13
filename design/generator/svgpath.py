"""SVG path helpers (from the OpsNow generator)."""
import re


def _num_tokens(d):
    return re.findall(r"[MmLlHhVvCcSsQqTtAaZz]|-?(?:\d+\.?\d*|\.\d+)(?:e-?\d+)?", d)


def scale_path(d, k):
    """Scale SVG path data by k, keeping arc rotation/flags."""
    out = []
    i = 0
    toks = []
    # arc flags can be packed ("011"), so tokenize arcs specially
    pos = 0
    pat = re.compile(r"\s*,?\s*([MmLlHhVvCcSsQqTtAaZz]|-?(?:\d+\.?\d*|\.\d+)(?:e-?\d+)?)")
    cur = None
    argi = 0
    lead = False
    while pos < len(d):
        m = pat.match(d, pos)
        if not m:
            pos += 1
            continue
        tok = m.group(1)
        if tok.isalpha():
            if not out and tok == "m":
                tok = "M"
                lead = True
            cur = tok
            argi = 0
            out.append(tok)
            pos = m.end()
            continue
        if cur == "M" and argi == 2 and lead:
            out.append("l")
            cur = "l"
            argi = 0
        if cur in "Aa" and argi % 7 in (3, 4):
            # single-char flag
            sm = re.compile(r"\s*,?\s*([01])").match(d, pos)
            out.append(sm.group(1))
            pos = sm.end()
            argi += 1
            continue
        v = float(tok)
        if cur in "Aa" and argi % 7 == 2:
            out.append(tok)
        else:
            out.append(("%.3f" % (v * k)).rstrip("0").rstrip("."))
        argi += 1
        pos = m.end()
    return " ".join(out)


def _attrs(s):
    return dict(re.findall(r'([a-z-]+)="([^"]*)"', s))


def svg_to_path(markup):
    parts = []
    for tag, attrs in re.findall(r"<(path|circle|rect|line|ellipse|polyline)([^>]*)/?>", markup):
        a = _attrs(attrs)
        if tag == "path":
            parts.append(a["d"].strip())
        elif tag == "circle":
            cx, cy, r = float(a["cx"]), float(a["cy"]), float(a["r"])
            k = 0.5523 * r
            parts.append(f"M{cx} {cy - r}C{cx + k} {cy - r} {cx + r} {cy - k} {cx + r} {cy}C{cx + r} {cy + k} {cx + k} {cy + r} {cx} {cy + r}"
                         f"C{cx - k} {cy + r} {cx - r} {cy + k} {cx - r} {cy}C{cx - r} {cy - k} {cx - k} {cy - r} {cx} {cy - r}Z")
        elif tag == "rect":
            x, y = float(a.get("x", 0)), float(a.get("y", 0))
            w, h = float(a["width"]), float(a["height"])
            rx = float(a.get("rx", 0))
            if rx:
                parts.append(f"M{x + rx} {y}h{w - 2 * rx}a{rx} {rx} 0 0 1 {rx} {rx}v{h - 2 * rx}a{rx} {rx} 0 0 1 {-rx} {rx}"
                             f"h{-(w - 2 * rx)}a{rx} {rx} 0 0 1 {-rx} {-rx}v{-(h - 2 * rx)}a{rx} {rx} 0 0 1 {rx} {-rx}z")
            else:
                parts.append(f"M{x} {y}h{w}v{h}h{-w}z")
        elif tag == "line":
            parts.append(f"M{a['x1']} {a['y1']}L{a['x2']} {a['y2']}")
    return parts


