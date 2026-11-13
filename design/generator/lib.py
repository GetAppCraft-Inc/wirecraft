"""Tiny DSL that emits a FigCraft `run` script (JSON array of commands) for the xwire design.

Colours are never hard-coded on layers: every fill/stroke is bound to a variable in the
`xwire/color` collection, which has a Dark and a Light mode. A frame picks its theme with
`theme(frame, "Light")`.
"""
import json
from bbox import bbox
from svgpath import scale_path, svg_to_path

FONT = "Inter"
MONO = "DejaVu Sans Mono"   # stand-in for JetBrains Mono (not bundled with FigCraft)
COLL = "xwire/color"

# ---------------------------------------------------------------- colour tokens  name: (dark, light)
TOKENS = [
    ("Background", [
        ("bg/app", "#0a0c0f", "#e9ecf0", "Window backdrop, gaps between panes"),
        ("bg/panel", "#111418", "#ffffff", "Pane surface: table, inspector, sidebar"),
        ("bg/raised", "#171b20", "#f6f7f9", "Toolbars, table header, cards"),
        ("bg/sunken", "#0c0f12", "#f1f3f6", "Inputs, code blocks"),
        ("bg/hover", "#1c2127", "#eef1f5", "Row and control hover"),
        ("bg/selected", "#232849", "#e3e6ff", "Selected row, active item"),
        ("bg/overlay", "#1b2026", "#ffffff", "Menus, modals, toasts"),
        ("bg/scrim", "#05070acc", "#0a0c0f66", "Behind modals"),
    ]),
    ("Border", [
        ("border/subtle", "#1e232a", "#e8ebef", "Row dividers"),
        ("border/default", "#2a3038", "#d9dde3", "Pane edges, inputs"),
        ("border/strong", "#3b434e", "#c1c7cf", "Focus-adjacent, menus"),
    ]),
    ("Text", [
        ("text/primary", "#e6e9ee", "#14171c", "Body, values"),
        ("text/secondary", "#a1aab6", "#4d5663", "Labels, header names"),
        ("text/tertiary", "#6a737f", "#8790a0", "Hints, line numbers, offsets"),
        ("text/on-accent", "#0a0c0f", "#ffffff", "Text on accent fills"),
    ]),
    ("Accent", [
        ("accent/default", "#8b93ff", "#4b52d6", "Primary action, focus ring, selection bar"),
        ("accent/hover", "#a5abff", "#3c42b5", ""),
        ("accent/soft", "#1e2242", "#ebecff", "Active chip, accent tint"),
        ("accent/text", "#b3b8ff", "#3c42b5", "Accent text on soft fills"),
    ]),
    ("Semantic", [
        ("success/default", "#3fcf8e", "#16855a", ""), ("success/soft", "#11281e", "#e2f4ea", ""),
        ("warning/default", "#f2b544", "#975f00", ""), ("warning/soft", "#2b2212", "#fbf0d9", ""),
        ("error/default", "#f2615a", "#c42e29", ""), ("error/soft", "#2c1516", "#fde8e7", ""),
        ("info/default", "#4fb3ff", "#1d6dbd", ""), ("info/soft", "#0f2236", "#e3f0fc", ""),
    ]),
    ("HTTP method", [
        ("method/get", "#4fb3ff", "#1d6dbd", ""), ("method/post", "#3fcf8e", "#16855a", ""),
        ("method/put", "#f2b544", "#975f00", ""), ("method/patch", "#c58fff", "#7a3cc4", ""),
        ("method/delete", "#f2615a", "#c42e29", ""), ("method/options", "#8a94a1", "#646e7b", ""),
    ]),
    ("Status code", [
        ("status/1xx", "#8a94a1", "#646e7b", "Informational, WebSocket upgrade"),
        ("status/2xx", "#3fcf8e", "#16855a", "Success"),
        ("status/3xx", "#3fd0d6", "#0a7f87", "Redirect, not modified"),
        ("status/4xx", "#f2b544", "#975f00", "Client error"),
        ("status/5xx", "#f2615a", "#c42e29", "Server error"),
    ]),
    ("Syntax & rows", [
        ("syntax/key", "#8cc5ff", "#1c5ea6", ""), ("syntax/string", "#a3dc8c", "#3a7a1f", ""),
        ("syntax/number", "#f5a872", "#b0521a", ""), ("syntax/keyword", "#c58fff", "#7a3cc4", ""),
        ("syntax/punct", "#7b8491", "#8790a0", ""),
        ("row/error", "#1d1214", "#fdf2f1", "Error row tint"),
        ("row/pinned", "#161a2c", "#f3f4ff", "Pinned row tint"),
    ]),
]

SHADOW = {
    1: [{"type": "dropShadow", "color": "#00000040", "offset": [0, 1], "radius": 2}],
    2: [{"type": "dropShadow", "color": "#00000059", "offset": [0, 6], "radius": 16}],
    3: [{"type": "dropShadow", "color": "#00000073", "offset": [0, 16], "radius": 40}],
}
METHOD_VAR = {"GET": "method/get", "POST": "method/post", "PUT": "method/put", "PATCH": "method/patch",
              "DELETE": "method/delete", "OPTIONS": "method/options", "HEAD": "method/options", "CONNECT": "method/options"}

C = []
_n = [0]
LAYOUT = set()


def col(c):
    """Token name → variable binding; '#hex' and 'none' pass through."""
    if c is None or c == "none":
        return "none"
    if isinstance(c, str) and c.startswith("#"):
        return c
    return {"$var": c}


def R(r):
    return {"$ref": r}


def nid():
    _n[0] += 1
    return "n%d" % _n[0]


def cmd(c, p=None, as_=None):
    d = {"command": c, "params": p or {}}
    if as_:
        d["as"] = as_
    C.append(d)


def define_tokens():
    cmd("variable.createCollection", {"name": COLL, "modes": ["Dark", "Light"]})
    for group, items in TOKENS:
        for name, dark, light, desc in items:
            p = {"collection": COLL, "name": name, "type": "color", "values": {"Dark": dark, "Light": light}}
            if desc:
                p["description"] = desc
            cmd("variable.create", p)
    cmd("variable.createCollection", {"name": "xwire/dimension"})
    for i in range(1, 13):
        cmd("variable.create", {"collection": "xwire/dimension", "name": "space/%d" % (i * 4), "type": "number", "value": i * 4})
    for nm, v in [("xs", 3), ("sm", 4), ("md", 6), ("lg", 8), ("xl", 12), ("full", 999)]:
        cmd("variable.create", {"collection": "xwire/dimension", "name": "radius/" + nm, "type": "number", "value": v})
    for nm, v in [("row", 24), ("row-compact", 20), ("control", 28), ("control-sm", 22), ("toolbar", 44), ("statusbar", 24)]:
        cmd("variable.create", {"collection": "xwire/dimension", "name": "size/" + nm, "type": "number", "value": v})


def token_hex(name, mode=0):
    for _, items in TOKENS:
        for n, d, l, _ in items:
            if n == name:
                return (d, l)[mode]


def theme(ref, mode):
    cmd("variable.setMode", {"ids": [R(ref)], "collection": COLL, "mode": mode})


def node(t, parent=None, name=None, w=None, h=None, x=None, y=None, text=None, **props):
    r = nid()
    p = {"type": t, "select": False}
    if parent:
        p["parent"] = R(parent)
    if name:
        p["name"] = name
    if w is not None:
        p["width"] = w
    if h is not None:
        p["height"] = h
    if x is not None:
        p["x"] = x
    if y is not None:
        p["y"] = y
    if text is not None:
        p["text"] = text
    if props:
        p["props"] = props
    cmd("node.create", p, r)
    return r


def setp(ref, **props):
    cmd("node.set", {"ids": [R(ref)], "props": props})


def box(parent=None, d="v", gap=0, pad=0, w=None, h=None, fill=None, name=None, sw=None, sh=None,
        main=None, cross=None, radius=None, stroke=None, sides=None, effects=None, clip=None, wrap=None,
        kind="frame", x=None, y=None, **extra):
    """Frame. d: 'v' | 'h' | None (no auto layout). sw/sh: fill | hug | fixed."""
    props = {"fill": col(fill)}
    if d:
        props["layout"] = "vertical" if d == "v" else "horizontal"
        props["gap"] = gap
        props["padding"] = pad
        if main:
            props["mainAlign"] = main
        if cross:
            props["crossAlign"] = cross
        if wrap:
            props["wrap"] = True
    in_layout = parent in LAYOUT
    if sw is None:
        sw = "fixed" if w is not None else ("hug" if d else "fixed")
    if sh is None:
        sh = "fixed" if h is not None else ("hug" if d else "fixed")
    if sw == "fill" and not in_layout:
        sw = "fixed"
    if sh == "fill" and not in_layout:
        sh = "fixed"
    if sw == "hug" and not d:
        sw = "fixed"
    if sh == "hug" and not d:
        sh = "fixed"
    props["sizingH"] = sw
    props["sizingV"] = sh
    if radius is not None:
        props["cornerRadius"] = radius
    if stroke:
        props["stroke"] = col(stroke)
        if sides:
            t, r_, b, l = sides
            props.update(strokeTopWeight=t, strokeRightWeight=r_, strokeBottomWeight=b, strokeLeftWeight=l)
        else:
            props["strokeWeight"] = 1
        props["strokeAlign"] = "inside"
    if effects:
        props["effects"] = effects
    if clip is not None:
        props["clip"] = clip
    props.update(extra)
    ref = node(kind, parent, name, w if w is not None else (100 if sw == "fixed" else None),
               h if h is not None else (100 if sh == "fixed" else None), x, y, **props)
    if d:
        LAYOUT.add(ref)
    return ref


def H(parent, gap=8, **kw):
    kw.setdefault("cross", "center")
    return box(parent, "h", gap, **kw)


def V(parent, gap=8, **kw):
    return box(parent, "v", gap, **kw)


def spacer(parent, w=None, h=None):
    """Flexible (fill) spacer in an auto-layout frame."""
    return box(parent, None, w=w or 1, h=h or 1, sw="fill" if w is None else "fixed",
               sh="fixed", name="Spacer")


def T(parent, s, size=13, wt="Regular", color="text/primary", sw=None, w=None, align=None, name=None, lh=None,
      upper=False, ls=None, mono=False, trunc=False):
    stripped = s.rstrip(" ")
    if stripped != s:  # trailing spaces are trimmed by text layout: keep them as no-break spaces
        s = stripped + "\u00a0" * (len(s) - len(stripped))
    if mono and wt in ("Medium", "Semi Bold"):
        wt = "Bold"
    props = {"fontFamily": MONO if mono else FONT, "fontStyle": wt, "fontSize": size, "fill": col(color)}
    if lh:
        props["lineHeight"] = lh
    if align:
        props["textAlign"] = align
    if upper:
        props["textCase"] = "upper"
    if ls is not None:
        props["letterSpacing"] = ls
    width = None
    if sw == "fill" and parent in LAYOUT:
        props["sizingH"] = "fill"
        props["textAutoResize"] = "truncate" if trunc else "height"
    elif w:
        width = w
        props["textAutoResize"] = "truncate" if trunc else "height"
    if trunc:
        props["maxLines"] = 1
    return node("text", parent, name or s[:40], w=width, text=s, **props)


def M(parent, s, size=12, color="text/primary", **kw):
    return T(parent, s, size, kw.pop("wt", "Regular"), color, mono=True, **kw)


def hr(parent, color="border/subtle", vertical=False):
    if vertical:
        return box(parent, None, w=1, sh="fill", fill=color, name="Divider")
    return box(parent, None, h=1, sw="fill", fill=color, name="Divider")


def dot(parent, color, size=6):
    return node("ellipse", parent, "dot", w=size, h=size, fill=col(color))


# ---------------------------------------------------------------- icons (original set, 24 px grid, stroked)
def _c(cx, cy, r):
    return '<circle cx="%s" cy="%s" r="%s"/>' % (cx, cy, r)


ICON_SRC = {
    "logo": '<path d="M4 4C10 4 14 20 20 20"/><path d="M4 20C10 20 14 4 20 4"/>',
    "pause": '<path d="M9 5v14"/><path d="M15 5v14"/>',
    "play": '<path d="M7 5l12 7-12 7z"/>',
    "trash": '<path d="M4 7h16"/><path d="M10 11v6"/><path d="M14 11v6"/><path d="M6 7l1 13h10l1-13"/><path d="M9 7V4h6v3"/>',
    "search": _c(11, 11, 7) + '<path d="M20 20l-4-4"/>',
    "filter": '<path d="M4 5h16l-6 8v6l-4-2v-4z"/>',
    "pin": '<path d="M9 4h6l-1 6 3 3H7l3-3z"/><path d="M12 13v7"/>',
    "star": '<path d="M12 4l2.5 5 5.5.8-4 3.9.9 5.5-4.9-2.6-4.9 2.6.9-5.5-4-3.9 5.5-.8z"/>',
    "folder": '<path d="M3 7h6l2 2h10v10H3z"/>',
    "file": '<path d="M6 3h8l4 4v14H6z"/><path d="M14 3v4h4"/>',
    "chevron-down": '<path d="M6 9l6 6 6-6"/>',
    "chevron-right": '<path d="M9 6l6 6-6 6"/>',
    "chevron-left": '<path d="M15 6l-6 6 6 6"/>',
    "lock": '<rect x="5" y="11" width="14" height="9" rx="2"/><path d="M8 11V8a4 4 0 0 1 8 0v3"/>',
    "unlock": '<rect x="5" y="11" width="14" height="9" rx="2"/><path d="M8 11V8a4 4 0 0 1 7.5-2"/>',
    "shield": '<path d="M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6z"/>',
    "shield-check": '<path d="M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6z"/><path d="M8.5 12l2.5 2.5 4.5-5"/>',
    "check": '<path d="M5 12l5 5 9-10"/>',
    "x": '<path d="M6 6l12 12"/><path d="M18 6L6 18"/>',
    "minus": '<path d="M5 12h14"/>',
    "square": '<rect x="5" y="5" width="14" height="14" rx="1"/>',
    "alert": '<path d="M12 4l9 16H3z"/><path d="M12 10v4"/><path d="M12 17v.5"/>',
    "info": _c(12, 12, 9) + '<path d="M12 11v5"/><path d="M12 8v.5"/>',
    "globe": _c(12, 12, 9) + '<path d="M3 12h18"/><path d="M12 3c3 3 3 15 0 18"/><path d="M12 3c-3 3-3 15 0 18"/>',
    "monitor": '<rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8"/><path d="M12 16v4"/>',
    "phone": '<rect x="7" y="3" width="10" height="18" rx="2"/><path d="M11 18h2"/>',
    "app": '<rect x="4" y="4" width="16" height="16" rx="4"/><path d="M9 12h6"/>',
    "terminal": '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M7 9l3 3-3 3"/><path d="M12 15h5"/>',
    "breakpoint": '<path d="M4 6h11l5 6-5 6H4z"/>',
    "map": '<path d="M4 6l5-2 6 2 5-2v14l-5 2-6-2-5 2z"/><path d="M9 4v14"/><path d="M15 6v14"/>',
    "remote": '<path d="M4 8h13l-3-3"/><path d="M20 16H7l3 3"/>',
    "rewrite": '<path d="M4 20h4L19 9l-4-4L4 16z"/><path d="M13 7l4 4"/>',
    "code": '<path d="M8 7l-5 5 5 5"/><path d="M16 7l5 5-5 5"/>',
    "send": '<path d="M4 12l16-8-6 16-3-6z"/><path d="M11 14l9-10"/>',
    "diff": '<path d="M7 4v16"/><path d="M17 4v16"/><path d="M4 8h6"/><path d="M14 16h6"/><path d="M17 13v6"/>',
    "repeat": '<path d="M4 12a8 8 0 0 1 14-5l2 2"/><path d="M20 4v5h-5"/><path d="M20 12a8 8 0 0 1-14 5l-2-2"/><path d="M4 20v-5h5"/>',
    "sliders": '<path d="M4 7h10"/><path d="M18 7h2"/><path d="M4 17h4"/><path d="M12 17h8"/>' + _c(16, 7, 2) + _c(10, 17, 2),
    "columns": '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M9 4v16"/><path d="M15 4v16"/>',
    "copy": '<rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V5H4v11h4"/>',
    "download": '<path d="M12 4v11"/><path d="M7 10l5 5 5-5"/><path d="M5 20h14"/>',
    "upload": '<path d="M12 15V4"/><path d="M7 9l5-5 5 5"/><path d="M5 20h14"/>',
    "clock": _c(12, 12, 9) + '<path d="M12 7v5l3 2"/>',
    "bolt": '<path d="M13 3L5 14h6l-1 7 8-11h-6z"/>',
    "wifi": '<path d="M4 9a11 11 0 0 1 16 0"/><path d="M7.5 12.5a6 6 0 0 1 9 0"/><path d="M12 16.5v.5"/>',
    "sidebar": '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M9 4v16"/>',
    "panel-bottom": '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 14h18"/>',
    "panel-right": '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M14 4v16"/>',
    "more": _c(6, 12, 1) + _c(12, 12, 1) + _c(18, 12, 1),
    "plus": '<path d="M12 5v14"/><path d="M5 12h14"/>',
    "layers": '<path d="M12 4l9 5-9 5-9-5z"/><path d="M3 14l9 5 9-5"/>',
    "cpu": '<rect x="6" y="6" width="12" height="12" rx="2"/><path d="M9 3v3"/><path d="M15 3v3"/><path d="M9 18v3"/><path d="M15 18v3"/><path d="M3 9h3"/><path d="M3 15h3"/><path d="M18 9h3"/><path d="M18 15h3"/>',
    "command": '<path d="M9 6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3z"/>',
    "settings": _c(12, 12, 3) + '<path d="M12 2v3"/><path d="M12 19v3"/><path d="M2 12h3"/><path d="M19 12h3"/><path d="M4.9 4.9l2.1 2.1"/><path d="M17 17l2.1 2.1"/><path d="M4.9 19.1L7 17"/><path d="M17 7l2.1-2.1"/>',
    "wrap": '<path d="M4 6h16"/><path d="M4 12h13a3 3 0 0 1 0 6h-4"/><path d="M15 16l-2 2 2 2"/><path d="M4 18h5"/>',
    "eye": '<path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12z"/>' + _c(12, 12, 3),
    "external": '<path d="M14 4h6v6"/><path d="M20 4l-9 9"/><path d="M18 14v6H4V6h6"/>',
    "keyboard": '<rect x="2" y="6" width="20" height="12" rx="2"/><path d="M6 10h.5"/><path d="M10 10h.5"/><path d="M14 10h.5"/><path d="M18 10h.5"/><path d="M7 14h10"/>',
    "qr": '<rect x="4" y="4" width="6" height="6"/><rect x="14" y="4" width="6" height="6"/><rect x="4" y="14" width="6" height="6"/><path d="M14 14h2v2"/><path d="M20 14v6h-6"/>',
    "image": '<rect x="3" y="5" width="18" height="14" rx="2"/>' + _c(8.5, 10, 1.5) + '<path d="M21 16l-5-5-9 8"/>',
    "plug": '<path d="M9 3v5"/><path d="M15 3v5"/><path d="M6 8h12v3a6 6 0 0 1-12 0z"/><path d="M12 17v4"/>',
    "record": _c(12, 12, 6),
    "history": '<path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5"/><path d="M12 7v5l3 2"/>',
    "throttle": '<path d="M4 18a8 8 0 1 1 16 0"/><path d="M12 18l4-6"/>',
    "link": '<path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1"/><path d="M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1"/>',
    "user": _c(12, 8, 4) + '<path d="M4 21a8 8 0 0 1 16 0"/>',
}
ICONS = {k: svg_to_path(v) for k, v in ICON_SRC.items()}


def icon(parent, name, size=14, color="text/secondary", weight=None, fill=None):
    f = box(parent, None, w=size, h=size, name="icon/" + name)
    k = size / 24.0
    for d in ICONS[name]:
        p = scale_path(d, k)
        x0, y0, _, _ = bbox(p)
        node("vector", f, name, x=round(x0, 3), y=round(y0, 3), path=p,
             fill=col(fill) if fill else "none", stroke=col(color),
             strokeWeight=weight or (1.6 if size >= 16 else 1.4), strokeCap="round", strokeJoin="round")
    return f


def logo(parent, size=20, wordmark=True, color="accent/default"):
    r = H(parent, 8, name="xwire logo")
    m = box(r, None, w=size, h=size, name="mark")
    k = size / 24.0
    for d in ICONS["logo"]:
        p = scale_path(d, k)
        x0, y0, _, _ = bbox(p)
        node("vector", m, "wire", x=round(x0, 3), y=round(y0, 3), path=p, fill="none", stroke=col(color),
             strokeWeight=max(1.6, size / 9), strokeCap="round")
    dsz = max(4, size * 0.28)
    node("ellipse", m, "node", w=dsz, h=dsz, x=(size - dsz) / 2, y=(size - dsz) / 2, fill=col("text/primary"))
    if wordmark:
        T(r, "xwire", max(13, size * 0.72), "Bold", "text/primary", ls=-0.3)
    return r


def save(path):
    with open(path, "w") as fh:
        json.dump(C, fh)
    return len(C)
