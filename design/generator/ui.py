"""xwire components. Everything binds to tokens in lib.TOKENS; sizes follow the 4 px grid."""
import json
import random
import re
from lib import *


# ---------------------------------------------------------------- small controls
def kbd(parent, keys, size=10.5):
    r = H(parent, 2, name="Kbd/" + "+".join(keys))
    for k in keys:
        b = H(r, 0, pad=[1, 4], radius=3, fill="bg/sunken", stroke="border/default", main="center", name="key")
        setp(b, minWidth=16)
        T(b, k, size, "Medium", "text/secondary")
    return r


def btn(parent, label=None, kind="secondary", ic=None, size="md", keys=None, w=None):
    h = {"md": 28, "sm": 22, "lg": 34}[size]
    fill, fg, stroke = {
        "primary": ("accent/default", "text/on-accent", None),
        "secondary": ("bg/raised", "text/primary", "border/default"),
        "ghost": (None, "text/secondary", None),
        "danger": (None, "error/default", "border/default"),
        "active": ("accent/soft", "accent/text", None),
    }[kind]
    padx = 12 if label else (6 if size != "sm" else 4)
    b = H(parent, 6, pad=[0, padx], h=h, w=w, fill=fill, radius=6 if size != "sm" else 4, stroke=stroke,
          main="center", name="Button/" + (label or ic))
    if ic:
        icon(b, ic, 14 if size != "lg" else 16, fg)
    if label:
        T(b, label, 12 if size == "sm" else 13, "Medium", fg)
    if keys:
        k = kbd(b, keys)
        setp(k, opacity=0.8)
    return b


def icon_btn(parent, ic, active=False, size=28, color=None, badge=None):
    b = H(parent, 0, w=size, h=size, radius=6, fill="accent/soft" if active else None, main="center",
          name="IconButton/" + ic)
    icon(b, ic, 16 if size >= 28 else 14, color or ("accent/text" if active else "text/secondary"))
    return b


def seg(parent, options, active=0, size="md", icons=None, w=None):
    h = 26 if size == "md" else 22
    s = H(parent, 2, pad=2, h=h, w=w, radius=6, fill="bg/sunken", stroke="border/subtle", name="Segmented")
    for i, o in enumerate(options):
        on = i == active
        it = H(s, 6, pad=[0, 10 if size == "md" else 8], sh="fill", sw="fill" if w else None, radius=4,
               fill="bg/hover" if on else None, main="center", effects=SHADOW[1] if on else None, name="Segment/" + o)
        if icons and icons[i]:
            icon(it, icons[i], 13, "text/primary" if on else "text/tertiary")
        if o:
            T(it, o, 12, "Medium", "text/primary" if on else "text/secondary")
    return s


def toggle(parent, on=True, size=1.0):
    w, h = int(30 * size), int(18 * size)
    t = box(parent, "h", w=w, h=h, radius=h / 2, fill="accent/default" if on else "border/strong", pad=2,
            main="max" if on else "min", cross="center", name="Toggle/" + ("on" if on else "off"))
    node("ellipse", t, "knob", w=h - 4, h=h - 4, fill="#ffffff")
    return t


def checkbox(parent, on=False):
    c = box(parent, "h", w=14, h=14, radius=3, fill="accent/default" if on else "bg/sunken",
            stroke=None if on else "border/strong", main="center", cross="center", name="Checkbox")
    if on:
        icon(c, "check", 11, "text/on-accent", 2.2)
    return c


def radio(parent, on=False):
    r = box(parent, "h", w=14, h=14, radius=7, fill="bg/sunken", stroke="accent/default" if on else "border/strong",
            main="center", cross="center", name="Radio")
    if on:
        node("ellipse", r, "dot", w=6, h=6, fill=col("accent/default"))
    return r


def pill(parent, label, fg="text/secondary", bg="bg/hover", dot_=False, mono=False, stroke=None, size=11):
    p = H(parent, 5, pad=[1, 6], h=18, fill=bg, radius=4, stroke=stroke, name="Badge/" + label)
    if dot_:
        dot(p, fg, 6)
    T(p, label, size, "Medium", fg, mono=mono)
    return p


def method_badge(parent, m, solid=False):
    v = METHOD_VAR.get(m, "method/options")
    if solid:
        return pill(parent, m, v, "bg/sunken", mono=True, stroke=v, size=10.5)
    return T(parent, m, 11, "Bold", v, mono=True, name="Method/" + m)


def status_var(code):
    if not str(code).isdigit():
        return "text/tertiary"
    return "status/%sxx" % str(code)[0]


def status_badge(parent, code, text=None):
    v = status_var(code)
    return pill(parent, "%s %s" % (code, text) if text else str(code), v, "bg/sunken", dot_=True, mono=True, stroke="border/subtle")


def chip(parent, label, on=False, count=None, ic=None, tone=None):
    c = H(parent, 5, pad=[0, 8], h=22, radius=11, fill="accent/soft" if on else None,
          stroke="accent/default" if on else "border/default", name="Chip/" + label)
    if ic:
        icon(c, ic, 12, "accent/text" if on else (tone or "text/secondary"))
    T(c, label, 11.5, "Medium", "accent/text" if on else (tone or "text/secondary"))
    if count is not None:
        T(c, str(count), 10.5, "Medium", tone or ("accent/text" if on else "text/tertiary"), mono=True)
    return c


def text_input(parent, value="", placeholder="", w=None, mono=False, ic=None, right=None, focus=False, h=28, sw=None):
    f = H(parent, 6, pad=[0, 8], h=h, w=w, sw=sw or (None if w else "fill"), radius=6, fill="bg/sunken",
          stroke="accent/default" if focus else "border/default", name="Input")
    if focus:
        setp(f, effects=[{"type": "dropShadow", "color": token_hex("accent/default") + "40", "offset": [0, 0], "radius": 0, "spread": 3}])
    if ic:
        icon(f, ic, 14, "text/tertiary")
    T(f, value or placeholder, 12.5 if not mono else 12, "Regular", "text/primary" if value else "text/tertiary",
      sw="fill", trunc=True, mono=mono)
    if right == "chevron":
        icon(f, "chevron-down", 14, "text/tertiary")
    elif isinstance(right, str) and right:
        T(f, right, 11, "Medium", "success/default")
    return f


QUERY_COLORS = {"key": "accent/text", "op": "syntax/keyword", "val": "syntax/number", "str": "syntax/string",
                "plain": "text/primary", "neg": "error/default", "glob": "syntax/key"}


def query_field(parent, tokens, w=None, focus=False, count=None, sw=None):
    """Search field with query syntax highlighting. tokens: [(text, kind)]."""
    f = H(parent, 6, pad=[0, 8], h=28, w=w, sw=sw or (None if w else "fill"), radius=6, fill="bg/sunken",
          stroke="accent/default" if focus else "border/default", name="QueryField", clip=True)
    icon(f, "filter", 14, "accent/text" if tokens else "text/tertiary")
    q = H(f, 0, sw="fill", name="query", clip=True)
    if not tokens:
        T(q, "Filter  —  status:5xx  host:*.api.com  method:POST  body:\"error\"", 12, "Regular", "text/tertiary", mono=True)
    merged = merge_ws(tokens)
    for txt, kind in merged:
        M(q, txt, 12, QUERY_COLORS[kind])
    if focus:
        box(q, None, w=1.5, h=15, fill="accent/default", name="caret")
    if count:
        T(f, count, 11, "Medium", "text/tertiary", mono=True)
    kbd(f, ["Ctrl", "L"])
    return f


def tabs(parent, items, active=0, size="md", sw="fill", counts=None, right=None, pad=None):
    """Underline tabs. items: labels; counts: {label: n}."""
    h = 32 if size == "md" else 28
    t = H(parent, 0, h=h, sw=sw, pad=pad if pad is not None else [0, 8], stroke="border/subtle", sides=[0, 0, 1, 0],
          cross="max", name="Tabs")
    for i, it in enumerate(items):
        on = i == active
        tb = H(t, 5, pad=[0, 8], sh="fill", stroke="accent/default" if on else None,
               sides=[0, 0, 2, 0] if on else None, name="Tab/" + it)
        T(tb, it, 12, "Medium" if on else "Regular", "text/primary" if on else "text/secondary")
        if counts and it in counts:
            T(tb, str(counts[it]), 10.5, "Medium", "text/tertiary", mono=True)
    if right:
        spacer(t)
        r = H(t, 4, sh="fill", name="tab tools")
        right(r)
    return t


def section_label(parent, s, right=None):
    r = H(parent, 6, pad=[0, 12], h=26, sw="fill", name="SectionLabel/" + s)
    T(r, s, 10.5, "Semi Bold", "text/tertiary", upper=True, ls=0.6, sw="fill")
    if right:
        T(r, right, 10.5, "Medium", "text/tertiary", mono=True)
    return r


def tree_item(parent, label, depth=0, ic=None, count=None, expanded=None, selected=False, hover=False,
              color=None, mono=False, ic_color=None, right_icon=None):
    r = H(parent, 6, pad=[0, 10, 0, 10 + depth * 14], h=24, sw="fill", radius=5,
          fill="bg/selected" if selected else ("bg/hover" if hover else None), name="TreeItem/" + label)
    if expanded is None:
        box(r, None, w=12, h=12, name="no-chevron")
    else:
        icon(r, "chevron-down" if expanded else "chevron-right", 12, "text/tertiary")
    if ic:
        icon(r, ic, 14, ic_color or ("accent/text" if selected else "text/secondary"))
    T(r, label, 12, "Medium" if selected else "Regular", color or ("text/primary"), sw="fill", trunc=True, mono=mono)
    if right_icon:
        icon(r, right_icon, 12, "text/tertiary")
    if count is not None:
        T(r, count, 10.5, "Regular", "text/tertiary", mono=True)
    return r


def split_handle(parent, vertical=True, hover=False):
    """vertical=True: a handle between side-by-side panes."""
    if vertical:
        h = box(parent, "v", w=5 if hover else 1, sh="fill", fill="accent/default" if hover else "border/default",
                main="center", cross="center", name="SplitHandle/vertical")
    else:
        h = box(parent, "h", h=5 if hover else 1, sw="fill", fill="accent/default" if hover else "border/default",
                main="center", cross="center", name="SplitHandle/horizontal")
    return h


def grabber(parent, vertical=True):
    """Split handle detail: hairline with a 3-dot grip, for the components sheet."""
    if vertical:
        g = V(parent, 3, w=9, h=40, radius=4, fill="bg/hover", stroke="border/strong", main="center", cross="center", name="Grip")
    else:
        g = H(parent, 3, w=40, h=9, radius=4, fill="bg/hover", stroke="border/strong", main="center", cross="center", name="Grip")
    for _ in range(3):
        dot(g, "text/tertiary", 2.5)
    return g


# ---------------------------------------------------------------- request table
COLS = [  # key, title, width (None = fill), align
    ("idx", "#", 44, "right"), ("tls", "", 18, None), ("method", "Method", 56, None), ("status", "Status", 42, None),
    ("host", "Host", 128, None), ("path", "Path", None, None), ("type", "Type", 44, None),
    ("size", "Size", 54, "right"), ("dur", "Duration", 58, "right"), ("time", "Time", 62, None), ("client", "Client", 64, None),
]


def table_header(parent, cols=None, sort="idx"):
    cols = cols or [c[0] for c in COLS]
    h = H(parent, 0, pad=[0, 8], h=26, sw="fill", fill="bg/raised", stroke="border/default", sides=[0, 0, 1, 0], name="TableHeader")
    for key, title, w, align in COLS:
        if key not in cols:
            continue
        c = H(h, 3, pad=[0, 6], w=w, sw="fill" if w is None else None, sh="fill",
              main="max" if align == "right" else None, name="Col/" + (title or key))
        if key == "tls":
            icon(c, "lock", 11, "text/tertiary")
            continue
        T(c, title, 11, "Medium", "text/primary" if key == sort else "text/secondary")
        if key == sort:
            icon(c, "chevron-down", 10, "accent/text")
    icon_btn(h, "columns", size=22)
    return h


def table_row(parent, r, state="default", cols=None, h=24):
    """r: dict idx tls method status host path type size dur time client; state: default|hover|selected|pinned|error."""
    cols = cols or [c[0] for c in COLS]
    code = str(r["status"])
    err = code[:1] == "5" or state == "error"
    fill = {"default": None, "hover": "bg/hover", "selected": "bg/selected", "pinned": "row/pinned", "error": "row/error"}[state]
    if state == "default" and err:
        fill = "row/error"
    row = H(parent, 0, pad=[0, 8], h=h, sw="fill", fill=fill, stroke="accent/default" if state == "selected" else "border/subtle",
            sides=[0, 0, 0, 2] if state == "selected" else [0, 0, 1, 0], name="Row/%s" % r["idx"])
    base = "text/primary"
    for key, title, w, align in COLS:
        if key not in cols:
            continue
        c = H(row, 4, pad=[0, 6], w=w, sw="fill" if w is None else None, sh="fill",
              main="max" if align == "right" else None, name="Cell/" + key, clip=True)
        if key == "idx":
            if r.get("pinned"):
                icon(c, "pin", 11, "accent/text")
            if r.get("bp"):
                icon(c, "breakpoint", 11, "warning/default")
            M(c, str(r["idx"]), 11, "text/tertiary")
        elif key == "tls":
            if r.get("tls", True):
                icon(c, "lock", 11, "text/tertiary" if not r.get("pinned_cert") else "warning/default")
            else:
                icon(c, "unlock", 11, "warning/default")
        elif key == "method":
            method_badge(c, r["method"])
        elif key == "status":
            if code == "…":
                dot(c, "info/default", 6)
                M(c, "···", 11, "text/tertiary")
            elif code == "BP":
                M(c, "PAUSED", 9.5, "warning/default", wt="Bold")
            else:
                M(c, code, 11.5, status_var(code), wt="Bold" if err or code[0] == "4" else "Regular")
        elif key == "host":
            M(c, r["host"], 11.5, "error/default" if err else base, sw="fill", trunc=True)
        elif key == "path":
            M(c, r["path"], 11.5, "error/default" if err else ("text/primary" if state != "default" else "text/secondary"),
              sw="fill", trunc=True)
            if r.get("live"):
                pill(c, r["live"], "success/default", "success/soft", dot_=True, size=10)
        elif key == "type":
            T(c, r["type"], 11, "Regular", "text/tertiary")
        elif key == "size":
            M(c, r["size"], 11, "text/secondary")
        elif key == "dur":
            d = r["dur"]
            slow = d.endswith(" s")
            M(c, d, 11, "warning/default" if slow else "text/secondary")
        elif key == "time":
            M(c, r["time"], 11, "text/tertiary")
        elif key == "client":
            T(c, r["client"], 11, "Regular", "text/secondary", sw="fill", trunc=True)
    box(row, None, w=22, h=1, name="col-picker gutter")
    return row


# ---------------------------------------------------------------- code, JSON tree, hex
_TOK = re.compile(r'("(?:[^"\\]|\\.)*")(\s*:)?|(-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)|(true|false|null)|([{}\[\],:])|(\s+)|(.)')


def json_spans(line):
    spans = []
    for m in _TOK.finditer(line):
        s, colon, num, kw, punct, ws, other = m.groups()
        if s is not None:
            spans.append((s, "syntax/key" if colon else "syntax/string"))
            if colon:
                spans.append((colon, "syntax/punct"))
        elif num:
            spans.append((num, "syntax/number"))
        elif kw:
            spans.append((kw, "syntax/keyword"))
        elif punct:
            spans.append((punct, "syntax/punct"))
        elif ws:
            spans.append((ws, "text/primary"))
        else:
            spans.append((other, "text/primary"))
    return spans


def merge_ws(spans):
    """Whitespace-only text layers collapse to zero width: fold them into the neighbouring span."""
    out, pend = [], ""
    for t, tok in spans:
        if t.strip() == "":
            pend += t
            continue
        out.append((pend + t, tok))
        pend = ""
    if pend and out:
        out[-1] = (out[-1][0] + pend, out[-1][1])
    return out


def code_block(parent, lines, start=1, gutter=True, h=None, sw="fill", sh=None, highlight=None, size=11.5, name="CodeBlock",
               fill="bg/sunken", radius=6, stroke="border/subtle", pad=None, err_line=None):
    """lines: list of str (JSON-highlighted) or list of [(text, token)] spans."""
    b = V(parent, 0, pad=pad if pad is not None else [8, 0], sw=sw, h=h, sh=sh, fill=fill, radius=radius,
          stroke=stroke, clip=True, name=name)
    for i, ln in enumerate(lines):
        spans = merge_ws(json_spans(ln) if isinstance(ln, str) else ln)
        on = highlight == i
        row = H(b, 0, h=17, sw="fill", pad=[0, 10, 0, 0], fill="accent/soft" if on else ("error/soft" if err_line == i else None),
                name="line %d" % (start + i))
        if gutter:
            g = H(row, 0, w=38, sh="fill", main="max", pad=[0, 10, 0, 0], name="gutter")
            M(g, str(start + i), size - 1, "text/tertiary")
        else:
            box(row, None, w=10, h=1)
        for txt, tok in spans:
            M(row, txt, size, tok)
    return b


def sh(*parts):
    """Shell line → spans: first word as keyword, flags as key, quoted as string."""
    out = []
    for p in parts:
        out.append(p)
    return out


def shell_spans(line):
    spans = []
    for i, w in enumerate(re.split(r"(\s+)", line)):
        if not w:
            continue
        if w.isspace():
            spans.append((w, "text/primary"))
        elif w.startswith("$") or w == "#":
            spans.append((w, "text/tertiary"))
        elif w.startswith("-"):
            spans.append((w, "syntax/key"))
        elif w.startswith('"') or w.startswith("'"):
            spans.append((w, "syntax/string"))
        elif w in ("sudo", "security", "certutil", "update-ca-certificates", "trust", "cp", "adb", "xwire", "xcrun", "&&", "export", "curl"):
            spans.append((w, "syntax/keyword"))
        elif "/" in w or "." in w:
            spans.append((w, "syntax/string"))
        else:
            spans.append((w, "text/primary"))
    return spans


def shell_block(parent, cmds, sw="fill", copy=True):
    b = H(parent, 8, pad=[8, 10], sw=sw, fill="bg/sunken", radius=6, stroke="border/subtle", cross="min", name="ShellBlock")
    lines = V(b, 2, sw="fill")
    for c in cmds:
        r = H(lines, 0, sw="fill", name="cmd")
        M(r, "$ ", 11.5, "text/tertiary")
        for t, tok in merge_ws(shell_spans(c)):
            M(r, t, 11.5, tok)
    if copy:
        icon_btn(b, "copy", size=22)
    return b


def json_tree(parent, rows, sw="fill", h=None, sel=None):
    """rows: (depth, key, value, kind, expandable/expanded|None, meta)."""
    b = V(parent, 0, pad=[6, 0], sw=sw, h=h, fill="bg/sunken", radius=6, stroke="border/subtle", clip=True, name="JsonTree")
    for i, (depth, key, value, kind, exp, meta) in enumerate(rows):
        r = H(b, 4, h=20, sw="fill", pad=[0, 10, 0, 8 + depth * 16], fill="bg/selected" if sel == i else None, name="node/" + key)
        if exp is None:
            box(r, None, w=12, h=12)
        else:
            icon(r, "chevron-down" if exp else "chevron-right", 12, "text/tertiary")
        M(r, key, 11.5, "syntax/key")
        M(r, ":", 11.5, "syntax/punct")
        if kind in ("object", "array"):
            M(r, value, 11.5, "text/tertiary")
        else:
            M(r, value, 11.5, {"string": "syntax/string", "number": "syntax/number", "bool": "syntax/keyword", "null": "syntax/keyword"}[kind],
              sw="fill", trunc=True)
        if meta:
            spacer(r)
            T(r, meta, 10.5, "Regular", "text/tertiary")
    return b


def hex_view(parent, data, start=0, rows=8, sw="fill", sel=None):
    b = V(parent, 0, pad=[8, 0], sw=sw, fill="bg/sunken", radius=6, stroke="border/subtle", clip=True, name="HexView")
    hdr = H(b, 0, h=18, sw="fill", pad=[0, 10], name="hex header")
    M(hdr, "Offset    ", 11, "text/tertiary")
    M(hdr, " ".join("%02X" % i for i in range(8)) + "  " + " ".join("%02X" % i for i in range(8, 16)) + "   ", 11, "text/tertiary")
    M(hdr, "ASCII", 11, "text/tertiary")
    bs = data.encode()
    for r in range(rows):
        chunk = bs[(start + r) * 16:(start + r + 1) * 16]
        if not chunk:
            break
        row = H(b, 0, h=18, sw="fill", pad=[0, 10], fill="accent/soft" if sel == r else None, name="hex row")
        M(row, "%08x  " % ((start + r) * 16), 11, "text/tertiary")
        hx = " ".join("%02x" % c for c in chunk[:8]) + "  " + " ".join("%02x" % c for c in chunk[8:])
        M(row, hx.ljust(49) + "  ", 11, "text/primary")
        M(row, "".join(chr(c) if 32 <= c < 127 else "." for c in chunk), 11, "text/secondary")
    return b


def kv_table(parent, rows, key_w=150, sw="fill", mono=True, sel=None, editable=False):
    t = V(parent, 0, sw=sw, name="KeyValue")
    for i, (k, v) in enumerate(rows):
        r = H(t, 8, h=22, sw="fill", pad=[0, 12], cross="center", fill="bg/selected" if sel == i else None,
              stroke="border/subtle", sides=[0, 0, 1, 0], name="kv/" + k)
        if editable:
            checkbox(r, True)
        M(r, k, 11.5, "text/secondary", w=key_w, trunc=True)
        M(r, v, 11.5, "text/primary", sw="fill", trunc=True)
    return t


# ---------------------------------------------------------------- overlays
def menu(parent, items, w=232, x=None, y=None):
    """items: (icon, label, keys, kind) | '-' ; kind: None|'danger'|'sub'|'hover'|'header'."""
    m = V(parent, 0, pad=4, w=w, fill="bg/overlay", radius=8, stroke="border/strong", effects=SHADOW[2], name="ContextMenu", x=x, y=y)
    for it in items:
        if it == "-":
            d = H(m, 0, pad=[4, 0], sw="fill")
            hr(d, "border/default")
            continue
        ic, label, keys, kind = (list(it) + [None] * 4)[:4]
        if kind == "header":
            T(H(m, 0, pad=[6, 8, 2, 8], sw="fill"), label, 10.5, "Semi Bold", "text/tertiary", upper=True, ls=0.5)
            continue
        r = H(m, 8, pad=[0, 8], h=26, sw="fill", radius=5, fill="accent/default" if kind == "hover" else None, name="MenuItem/" + label)
        fg = "text/on-accent" if kind == "hover" else ("error/default" if kind == "danger" else "text/primary")
        if ic:
            icon(r, ic, 14, fg if kind in ("hover", "danger") else "text/secondary")
        else:
            box(r, None, w=14, h=14)
        T(r, label, 12.5, "Regular", fg, sw="fill")
        if keys:
            r2 = H(r, 2)
            for k in keys:
                T(r2, k, 11, "Regular", "text/on-accent" if kind == "hover" else "text/tertiary")
        if kind == "sub":
            icon(r, "chevron-right", 12, "text/tertiary")
    return m


def toast(parent, kind, title, body=None, action=None, w=340, x=None, y=None):
    tone = {"success": "success/default", "error": "error/default", "info": "info/default", "warning": "warning/default"}[kind]
    ic = {"success": "check", "error": "alert", "info": "info", "warning": "alert"}[kind]
    t = H(parent, 10, pad=[10, 12], w=w, fill="bg/overlay", radius=8, stroke="border/strong", effects=SHADOW[2],
          cross="min", name="Toast/" + kind, x=x, y=y)
    c = H(t, 0, w=20, h=20, radius=10, fill=kind + "/soft", main="center")
    icon(c, ic, 12, tone, 2)
    tx = V(t, 2, sw="fill")
    T(tx, title, 12.5, "Medium", "text/primary")
    if body:
        T(tx, body, 12, "Regular", "text/secondary", sw="fill")
    if action:
        T(t, action, 12, "Medium", "accent/text")
    icon(t, "x", 12, "text/tertiary")
    return t


def empty_state(parent, title, body, actions=(), ic="bolt", sw="fill", sh="fill"):
    e = V(parent, 12, sw=sw, sh=sh, main="center", cross="center", pad=24, name="EmptyState")
    c = H(e, 0, w=44, h=44, radius=12, fill="bg/raised", stroke="border/default", main="center")
    icon(c, ic, 20, "text/secondary")
    T(e, title, 14, "Semi Bold", "text/primary", align="center")
    T(e, body, 12.5, "Regular", "text/secondary", align="center", w=340, lh="150%")
    if actions:
        r = H(e, 8)
        for a in actions:
            btn(r, *a)
    return e


def modal(parent, title, body_fn, actions, w=520, x=None, y=None):
    m = V(parent, 0, w=w, fill="bg/overlay", radius=10, stroke="border/strong", effects=SHADOW[3], name="Modal/" + title,
          x=x, y=y, clip=True)
    hd = H(m, 8, pad=[14, 16], sw="fill", stroke="border/subtle", sides=[0, 0, 1, 0])
    T(hd, title, 14, "Semi Bold", "text/primary", sw="fill")
    kbd(hd, ["Esc"])
    bd = V(m, 12, pad=16, sw="fill")
    body_fn(bd)
    ft = H(m, 8, pad=[12, 16], sw="fill", fill="bg/raised", stroke="border/subtle", sides=[1, 0, 0, 0], main="max")
    for a in actions:
        btn(ft, *a)
    return m


def tooltip(parent, label, keys=None, x=None, y=None):
    t = H(parent, 8, pad=[4, 8], fill="bg/overlay", radius=5, stroke="border/strong", effects=SHADOW[1], name="Tooltip", x=x, y=y)
    T(t, label, 11.5, "Regular", "text/primary")
    if keys:
        kbd(t, keys)
    return t


# ---------------------------------------------------------------- QR code (decorative, deterministic)
def qr(parent, seed="xwire", n=29, mod=5, name="QR code"):
    rnd = random.Random(seed)
    grid = [[rnd.random() < 0.47 for _ in range(n)] for _ in range(n)]

    def finder(ox, oy):
        for y in range(-1, 8):
            for x in range(-1, 8):
                X, Y = ox + x, oy + y
                if 0 <= X < n and 0 <= Y < n:
                    edge = x in (0, 6) or y in (0, 6)
                    core = 2 <= x <= 4 and 2 <= y <= 4
                    grid[Y][X] = (0 <= x <= 6 and 0 <= y <= 6) and (edge or core)
    finder(0, 0)
    finder(n - 7, 0)
    finder(0, n - 7)
    for i in range(8, n - 8):
        grid[6][i] = grid[i][6] = i % 2 == 0
    q = mod * 2
    f = box(parent, None, w=n * mod + 2 * q, h=n * mod + 2 * q, fill="#ffffff", radius=8, name=name)
    for y in range(n):
        x = 0
        while x < n:
            if grid[y][x]:
                s = x
                while x < n and grid[y][x]:
                    x += 1
                node("rectangle", f, "m", w=(x - s) * mod, h=mod, x=q + s * mod, y=q + y * mod, fill="#0a0c0f")
            else:
                x += 1
    return f
