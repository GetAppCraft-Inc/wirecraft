"""Design system page: tokens, type, spacing/radius/elevation, and the component sheet (Dark + Light)."""
from workspace import *

MAKE_COMPONENTS = []   # (ref, name) turned into main components on the Dark sheet
VARIANT_SETS = []      # [refs] combined as variants


def board(x, y, w, title, sub=None, mode="Dark", name=None):
    b = V(None, 24, pad=40, w=w, fill="bg/app", radius=16, name=name or title, x=x, y=y)
    hd = V(b, 6, sw="fill")
    T(hd, title, 28, "Bold", "text/primary", ls=-0.4)
    if sub:
        T(hd, sub, 14, "Regular", "text/secondary", w=w - 80)
    theme(b, mode)
    return b


def group(parent, title, note=None, sw="fill", w=None, gap=12):
    g = V(parent, gap, pad=20, sw=None if w else sw, w=w, radius=12, fill="bg/panel", stroke="border/default", name="Group/" + title)
    hd = H(g, 10, sw="fill")
    T(hd, title, 14, "Semi Bold", "text/primary")
    if note:
        T(hd, note, 12, "Regular", "text/tertiary", sw="fill")
    return g


def label(parent, s):
    return T(parent, s, 11, "Medium", "text/tertiary", upper=True, ls=0.5)


# ---------------------------------------------------------------- foundations
def colour_board(x, y):
    b = board(x, y, 1720, "Colour", "Every layer binds to a variable in the xwire/color collection (modes: Dark, Light). "
              "Swatches show both modes; no hex values are set on screen layers. The accent is a periwinkle violet chosen to "
              "stay clear of the HTTP method and status hues.")
    for gname, items in TOKENS:
        g = group(b, gname)
        row = H(g, 12, sw="fill", wrap=True, cross="min")
        setp(row, crossGap=12)
        for nm, dark, light, desc in items:
            c = V(row, 6, w=196, name="Token/" + nm)
            sw_ = H(c, 0, sw="fill", h=44, radius=8, clip=True, stroke="border/default")
            for mode in ("Dark", "Light"):
                half = box(sw_, None, sw="fill", h=44, fill=nm, name=mode)
                theme(half, mode)
            M(c, nm, 11.5, "text/primary", wt="Bold")
            M(c, "%s · %s" % (dark.upper(), light.upper()), 10.5, "text/tertiary")
            if desc:
                T(c, desc, 11, "Regular", "text/secondary", sw="fill")
    return b


def http_board(x, y):
    b = board(x, y, 1720, "HTTP semantics", "Method and status colours are tokens too, used as text colour in the table "
              "(dense, low-noise) and as outlined badges in the inspector.")
    row = H(b, 16, sw="fill", cross="min")
    for mode in ("Dark", "Light"):
        g = group(row, "Methods & status · " + mode)
        theme(g, mode)
        r = H(g, 16, sw="fill")
        for m in ("GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"):
            method_badge(r, m)
        r = H(g, 8, sw="fill")
        for m in ("GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"):
            method_badge(r, m, solid=True)
        r = H(g, 8, sw="fill", wrap=True)
        for code, t in [(101, "Switching"), (200, "OK"), (204, "No Content"), (304, "Not Modified"), (404, "Not Found"),
                        (429, "Too Many"), (502, "Bad Gateway")]:
            status_badge(r, code, t)
        r = H(g, 6, sw="fill", wrap=True)
        for t, fg, bg in [("JSON", "text/secondary", "bg/hover"), ("GraphQL", "syntax/keyword", "bg/hover"), ("gRPC", "info/default", "info/soft"),
                          ("WS · live", "success/default", "success/soft"), ("Paused", "warning/default", "warning/soft"),
                          ("SSL pinned", "warning/default", "warning/soft"), ("Mapped", "accent/text", "accent/soft"), ("Error", "error/default", "error/soft")]:
            pill(r, t, fg, bg, dot_=t in ("WS · live", "Paused"))
    return b


def type_board(x, y):
    b = board(x, y, 840, "Type", "Inter for UI; a monospace face for URLs, headers, bodies and code (JetBrains Mono in the app — "
              "rendered here with DejaVu Sans Mono, which FigCraft bundles). Density range 11–14 px.")
    g = group(b, "Scale")
    for nm, size, wt, mono, sample in [
            ("Display", 34, "Bold", False, "See every request."), ("Title", 18, "Semi Bold", False, "Install the root certificate"),
            ("Heading", 14, "Semi Bold", False, "Response · Body"), ("Body", 13, "Regular", False, "Routes HTTP and HTTPS from every app."),
            ("UI", 12, "Regular", False, "Find source…"), ("Label", 11, "Medium", False, "Duration"),
            ("Overline", 10.5, "Semi Bold", False, "SAVED SESSIONS"), ("Mono / Code", 12, "Regular", True, '"code": "gateway_timeout"'),
            ("Mono / Table", 11.5, "Regular", True, "api.payments.dev/v2/charges"), ("Mono / Small", 11, "Regular", True, "14:02:13  412 B  1.84 s")]:
        r = H(g, 16, sw="fill")
        T(r, nm, 12, "Medium", "text/secondary", w=110)
        M(r, "%s/%s" % (size, wt.replace("Semi Bold", "600").replace("Bold", "700").replace("Medium", "500").replace("Regular", "400")),
          10.5, "text/tertiary", w=70)
        t = T(r, sample, size, wt, "text/primary", mono=mono, upper=nm == "Overline", ls=0.6 if nm == "Overline" else None)
        cmd("style.create", {"type": "text", "name": "xwire/" + nm, "from": R(t)})
    return b


def space_board(x, y):
    b = board(x, y, 860, "Space, radius, elevation", "4 px grid. Rows 24 px (20 compact), controls 28 px, toolbar 44 px, status bar 24 px.")
    g = group(b, "Spacing · space/4 … space/48")
    r = H(g, 14, sw="fill", cross="max")
    for i in (4, 8, 12, 16, 20, 24, 32, 40, 48):
        c = V(r, 6, cross="center")
        box(c, None, w=i, h=i, fill="accent/default", radius=2)
        M(c, str(i), 10.5, "text/tertiary")
    g = group(b, "Radius")
    r = H(g, 16, sw="fill")
    for nm, v in [("xs", 3), ("sm", 4), ("md", 6), ("lg", 8), ("xl", 12), ("full", 999)]:
        c = V(r, 6, cross="center")
        box(c, None, w=56, h=40, radius=min(v, 20), fill="bg/raised", stroke="border/strong")
        M(c, "%s · %s" % (nm, v if v < 999 else "pill"), 10.5, "text/tertiary")
    g = group(b, "Elevation")
    r = H(g, 20, sw="fill", pad=[8, 0])
    for lvl, use in [(0, "Panes (border only)"), (1, "Active segment, kbd"), (2, "Menus, toasts, popovers"), (3, "Modals")]:
        c = V(r, 8, cross="center")
        box(c, None, w=120, h=64, radius=8, fill="bg/overlay" if lvl else "bg/panel", stroke="border/default", effects=SHADOW.get(lvl))
        T(c, "e%d · %s" % (lvl, use), 11, "Regular", "text/secondary")
    return b


# ---------------------------------------------------------------- component sheet
def component_sheet(x, y, mode):
    dark = mode == "Dark"
    b = board(x, y, 1720, "Components · " + mode, "Built from tokens only — the Light sheet is the same layers with the "
              "variable mode switched." + (" Main components live on this sheet." if dark else ""), mode=mode,
              name="Components / " + mode)

    def comp(ref, name):
        if dark:
            MAKE_COMPONENTS.append((ref, name))
        return ref

    g = group(b, "Toolbar", "Capture controls, tools with active counts, proxy & CA status, command palette")
    tb_wrap = V(g, 0, sw="fill", radius=8, stroke="border/default", clip=True)
    comp(toolbar(tb_wrap), "Toolbar")

    row = H(b, 16, sw="fill", cross="min")
    g = group(row, "Buttons")
    r = H(g, 8, wrap=True, sw="fill")
    comp(btn(r, "Primary", "primary", "record", keys=["↵"]), "Button/Primary")
    comp(btn(r, "Secondary", "secondary", "repeat"), "Button/Secondary")
    comp(btn(r, "Ghost", "ghost"), "Button/Ghost")
    comp(btn(r, "Delete", "danger", "trash"), "Button/Danger")
    btn(r, "Small", "secondary", size="sm")
    comp(icon_btn(r, "pause"), "IconButton")
    icon_btn(r, "breakpoint", active=True)
    label(g, "Segmented control")
    r = H(g, 10, wrap=True, sw="fill")
    comp(seg(r, ["Pretty", "Raw", "Tree", "Hex", "Preview"], 0, size="sm"), "Segmented")
    seg(r, ["App", "Domain", "Device"], 1)
    seg(r, ["", ""], 0, icons=["panel-right", "panel-bottom"])
    label(g, "Toggle · checkbox · radio · keyboard hint · tooltip")
    r = H(g, 12, wrap=True, sw="fill")
    toggle(r, True)
    toggle(r, False)
    checkbox(r, True)
    checkbox(r, False)
    radio(r, True)
    radio(r, False)
    comp(kbd(r, ["Ctrl", "Shift", "C"]), "Kbd")
    kbd(r, ["Space"])
    comp(tooltip(r, "Repeat request", ["R"]), "Tooltip")

    g = group(row, "Badges, pills & filter chips")
    label(g, "Filter chips — default · active · tone")
    r = H(g, 6, wrap=True, sw="fill")
    setp(r, crossGap=6)
    comp(chip(r, "XHR", True, "1.4k"), "Chip/Active")
    comp(chip(r, "JS", False, "212"), "Chip/Default")
    chip(r, "GraphQL", False, "86")
    chip(r, "gRPC", False, "27")
    chip(r, "WS", False, "3")
    chip(r, "Errors", False, "17", ic="alert", tone="error/default")
    chip(r, "host: *.payments.dev", True, ic="x")
    label(g, "Status & method")
    r = H(g, 6, wrap=True, sw="fill")
    for code, t in [(200, "OK"), (304, None), (404, None), (502, "Bad Gateway")]:
        status_badge(r, code, t)
    for m in ("GET", "POST", "DELETE"):
        method_badge(r, m, solid=True)
    label(g, "Search field with query syntax")
    comp(query_field(g, [], sw="fill"), "QueryField/Empty")
    query_field(g, QUERY, sw="fill", count="3 of 2,484")
    query_field(g, [("method", "key"), (":", "op"), ("POST", "val"), (" -", "neg"), ("status", "key"), (":", "op"), ("2xx", "val"),
                    (" duration", "key"), (":", "op"), (">500ms", "val"), (" ", "plain")], sw="fill", focus=True)
    lg = V(g, 4, sw="fill", pad=[4, 0])
    for k, v in [("Keys", "method status host path type size duration client header body tls app device"),
                 ("Operators", "key:value  -key:value (not)  >  <  ..  wildcards *  regex /…/  quoted \"phrases\""),
                 ("Combine", "space = AND · OR · ( ) groups · saved filters with ★")]:
        rr = H(lg, 8, sw="fill")
        T(rr, k, 11, "Medium", "text/secondary", w=72)
        M(rr, v, 11, "text/tertiary", sw="fill")

    row = H(b, 16, sw="fill", cross="min")
    g = group(row, "Tabs", w=520)
    comp(tabs(g, ["Headers", "Query", "Cookies", "Body", "Auth", "Timing"], 3, counts={"Headers": 11, "Cookies": 3}), "Tabs")
    tabs(g, ["macOS", "Windows", "Linux", "Firefox", "Android", "iOS"], 2)
    label(g, "Split-pane handle — rest · hover/drag")
    r = H(g, 24, sw="fill", h=72)
    p1 = H(r, 0, w=180, sh="fill", radius=6, stroke="border/default", clip=True)
    box(p1, None, sw="fill", sh="fill", fill="bg/panel")
    split_handle(p1)
    box(p1, None, sw="fill", sh="fill", fill="bg/panel")
    p2 = H(r, 0, w=180, sh="fill", radius=6, stroke="border/default", clip=True)
    box(p2, None, sw="fill", sh="fill", fill="bg/panel")
    split_handle(p2, hover=True)
    box(p2, None, sw="fill", sh="fill", fill="bg/panel")
    grabber(r)
    grabber(r, vertical=False)
    T(g, "Hit area 8 px · drag shows the accent bar · double-click resets · Ctrl+\\ toggles inspector layout",
      11.5, "Regular", "text/tertiary", sw="fill")

    g = group(row, "Tree view", w=300)
    tv = V(g, 1, sw="fill", name="TreeView")
    comp(tv, "TreeView")
    tree_item(tv, "Chrome", ic="app", expanded=True, count="1,204")
    tree_item(tv, "api.payments.dev", depth=1, ic="lock", count="86", mono=True, selected=True)
    tree_item(tv, "cdn.shopassets.net", depth=1, ic="lock", count="412", mono=True, hover=True)
    tree_item(tv, "myapp.dev", depth=1, ic="unlock", ic_color="warning/default", count="18", mono=True)
    tree_item(tv, "iPhone 16 Pro", ic="phone", expanded=False, count="302", right_icon="wifi")

    g = group(row, "Toasts & menu")
    r = H(g, 16, sw="fill", cross="min")
    tc = V(r, 8)
    comp(toast(tc, "success", "Copied as cURL", None, None, w=300), "Toast/Success")
    toast(tc, "error", "Upstream connection reset", "api.payments.dev · ECONNRESET after 1.8 s", "Retry", w=300)
    toast(tc, "info", "Breakpoint hit", "POST /v2/refunds is paused", "Open", w=300)
    comp(menu(r, [("copy", "Copy URL", ["Ctrl", "C"], None), ("terminal", "Copy as cURL", ["Ctrl", "⇧", "C"], "hover"),
                  ("repeat", "Repeat", ["R"], None), "-", ("trash", "Delete", ["Del"], "danger")], w=220), "ContextMenu")

    g = group(b, "Virtualized table row", "24 px · default · hover · selected · pinned · error · paused at breakpoint · in flight")
    t = V(g, 0, sw="fill", radius=8, stroke="border/default", clip=True, fill="bg/panel")
    table_header(t)
    var_rows = []
    for st, idx in [("default", 2453), ("hover", 2457), ("selected", 2458), ("pinned", 2455), ("error", 2469), ("default", 2475), ("default", 2481)]:
        r = next(rr for rr in ROWS if rr["idx"] == idx)
        ref = table_row(t, r, st)
        var_rows.append((ref, st if idx not in (2475, 2481) else ("paused" if idx == 2475 else "pending")))
    if dark:
        VARIANT_SETS.append(("TableRow", var_rows))

    row = H(b, 16, sw="fill", cross="min")
    g = group(row, "Code editor block", w=500)
    comp(code_block(g, RESPONSE_LINES[:12], highlight=4), "CodeBlock")
    shell_block(g, ["curl -x http://127.0.0.1:9090 https://xwire.check"])
    g = group(row, "JSON tree", w=440)
    comp(json_tree(g, JSON_TREE_ROWS[:11], sel=3), "JsonTree")
    g = group(row, "Hex view")
    comp(hex_view(g, json.dumps(RESPONSE_JSON), rows=9, sel=2), "HexView")

    row = H(b, 16, sw="fill", cross="min")
    g = group(row, "Modal", w=600)
    comp(modal(g, "Add breakpoint", _bp_body, [("Cancel", "ghost"), ("Add breakpoint", "primary", None, "md", ["Ctrl", "↵"])], w=560), "Modal")
    g = group(row, "Empty state")
    comp(empty_state(g, "No requests yet", "Point an app at 127.0.0.1:9090 or turn on the system proxy. Requests appear here as they happen.",
                     [("Turn on system proxy", "primary", "plug"), ("Connect a device", "secondary", "phone")], sh=None), "EmptyState")
    empty_state(g, "Nothing matches  status:5xx", "3 filters active. Clear them or widen the query.", [("Clear filters", "secondary", "x")],
                ic="filter", sh=None)
    return b


def _bp_body(bd):
    r = H(bd, 10, sw="fill")
    T(r, "Match", 12, "Medium", "text/secondary", w=72)
    query_field(r, [("method", "key"), (":", "op"), ("POST", "val"), (" host", "key"), (":", "op"), ("api.payments.dev", "glob"),
                    (" path", "key"), (":", "op"), ("/v2/refunds*", "glob")])
    r = H(bd, 10, sw="fill")
    T(r, "Pause on", 12, "Medium", "text/secondary", w=72)
    seg(r, ["Request", "Response", "Both"], 2)
    r = H(bd, 10, sw="fill")
    T(r, "Timeout", 12, "Medium", "text/secondary", w=72)
    text_input(r, "60 s, then continue unchanged", w=260)
    r = H(bd, 8, sw="fill")
    checkbox(r, True)
    T(r, "Also pause matching WebSocket messages", 12.5, "Regular", "text/primary")


def finish_components():
    for ref, name in MAKE_COMPONENTS:
        setp(ref, name=name)
        cmd("component.create", {"ids": [R(ref)]})
    for set_name, refs in VARIANT_SETS:
        for ref, st in refs:
            setp(ref, name="State=" + st.capitalize())
            cmd("component.create", {"ids": [R(ref)]})
        cmd("component.combineVariants", {"ids": [R(r) for r, _ in refs]})
