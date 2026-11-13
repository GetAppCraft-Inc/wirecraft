"""Main workspace: toolbar, sidebar, filter bar, request table, inspector, status bar."""
import json
from ui import *

# ---------------------------------------------------------------- sample capture
_R = [
    # idx, method, status, host, path, type, size, dur, time, client, extra
    (2453, "GET", 200, "cdn.shopassets.net", "/static/js/app.4f2a1c.js", "JS", "312 KB", "84 ms", "14:02:11", "Chrome", {}),
    (2454, "GET", 200, "cdn.shopassets.net", "/static/css/main.9b1e.css", "CSS", "48 KB", "41 ms", "14:02:11", "Chrome", {}),
    (2455, "POST", 200, "api.payments.dev", "/v2/sessions", "JSON", "1.1 KB", "212 ms", "14:02:12", "Chrome", {"pinned": True}),
    (2456, "GET", 304, "fonts.gstatic.com", "/s/inter/v13/latin-ext.woff2", "Font", "0 B", "18 ms", "14:02:12", "Chrome", {}),
    (2457, "OPTIONS", 204, "api.payments.dev", "/v2/charges", "—", "0 B", "32 ms", "14:02:13", "Chrome", {}),
    (2458, "POST", 502, "api.payments.dev", "/v2/charges", "JSON", "412 B", "1.84 s", "14:02:13", "Chrome", {}),
    (2459, "POST", 200, "graphql.shop.dev", "/graphql  query CartLines", "GQL", "6.3 KB", "148 ms", "14:02:13", "Shop · iPhone", {}),
    (2460, "GET", 101, "rt.slack.dev", "/websocket/v3?token=xoxc-…", "WS", "—", "live", "14:02:14", "Slack", {"live": "214 frames"}),
    (2461, "POST", 200, "grpc.maps.dev", "/maps.v1.Routes/Compute", "gRPC", "2.4 KB", "96 ms", "14:02:14", "Maps · Pixel 9", {}),
    (2462, "GET", 404, "api.github.com", "/repos/xwire-dev/xwire/issues/0", "JSON", "120 B", "66 ms", "14:02:15", "curl", {}),
    (2463, "GET", 200, "images.picsum.dev", "/id/1015/1600/900.webp", "Img", "184 KB", "230 ms", "14:02:15", "Chrome", {}),
    (2464, "PUT", 200, "api.myapp.dev", "/v1/users/42/profile", "JSON", "820 B", "120 ms", "14:02:16", "Chrome", {}),
    (2465, "DELETE", 204, "api.myapp.dev", "/v1/sessions/9f2c", "—", "0 B", "54 ms", "14:02:16", "Chrome", {}),
    (2466, "PATCH", 200, "api.myapp.dev", "/v1/settings/notifications", "JSON", "304 B", "77 ms", "14:02:17", "Chrome", {"pinned": True}),
    (2467, "GET", 200, "spclient.music.dev", "/metadata/4/track/7ab2…", "JSON", "3.2 KB", "88 ms", "14:02:17", "Music", {}),
    (2468, "GET", 206, "audio-cdn.music.dev", "/audio/9a1f0c…?range=0-", "Media", "2.1 MB", "1.20 s", "14:02:18", "Music", {}),
    (2469, "POST", 500, "api.payments.dev", "/v2/webhooks/test", "JSON", "98 B", "2.40 s", "14:02:18", "curl", {}),
    (2470, "GET", 301, "myapp.dev", "/", "HTML", "0 B", "12 ms", "14:02:19", "Chrome", {"tls": False}),
    (2471, "GET", 200, "myapp.dev", "/dashboard", "HTML", "24 KB", "160 ms", "14:02:19", "Chrome", {}),
    (2472, "GET", 200, "cdn.shopassets.net", "/img/hero@2x.avif", "Img", "96 KB", "70 ms", "14:02:19", "Chrome", {}),
    (2473, "POST", 200, "events.telemetry.dev", "/v1/batch", "JSON", "22 B", "45 ms", "14:02:20", "Chrome", {}),
    (2474, "GET", 200, "api.github.com", "/user", "JSON", "1.4 KB", "102 ms", "14:02:20", "curl", {}),
    (2475, "POST", "BP", "api.payments.dev", "/v2/refunds", "JSON", "—", "—", "14:02:21", "Chrome", {"bp": True}),
    (2476, "POST", 200, "graphql.shop.dev", "/graphql  mutation AddToCart", "GQL", "1.9 KB", "91 ms", "14:02:21", "Shop · iPhone", {}),
    (2477, "GET", 429, "api.weather.dev", "/data/3.0/onecall?lat=12.97", "JSON", "210 B", "40 ms", "14:02:22", "Weather", {}),
    (2478, "GET", 200, "cdn.shopassets.net", "/static/js/vendor.81c0.js", "JS", "880 KB", "140 ms", "14:02:22", "Chrome", {}),
    (2479, "GET", 200, "api.myapp.dev", "/v1/feed?cursor=eyJpZCI6…", "JSON", "18 KB", "210 ms", "14:02:23", "Chrome", {}),
    (2480, "CONNECT", 200, "accounts.vendor.dev", ":443  (SSL pinned · tunnelled)", "TLS", "—", "—", "14:02:23", "Bank app", {"pinned_cert": True}),
    (2481, "GET", "…", "api.myapp.dev", "/v1/notifications", "JSON", "—", "—", "14:02:24", "Chrome", {}),
    (2482, "GET", "…", "images.picsum.dev", "/id/1025/400/400.webp", "Img", "—", "—", "14:02:24", "Chrome", {}),
    (2483, "POST", 201, "api.myapp.dev", "/v1/orders", "JSON", "640 B", "180 ms", "14:02:24", "Chrome", {}),
    (2484, "GET", 200, "cdn.shopassets.net", "/static/js/chunk-cart.js", "JS", "64 KB", "38 ms", "14:02:25", "Chrome", {}),
]
ROWS = []
for idx, m, st, host, path, typ, size, dur, tm, client, ex in _R:
    d = dict(idx=idx, method=m, status=st, host=host, path=path, type=typ, size=size, dur=dur, time=tm, client=client)
    d.update(ex)
    ROWS.append(d)
SELECTED = 2458
HOVER = 2457

RESPONSE_JSON = {
    "error": {
        "type": "upstream_error",
        "code": "gateway_timeout",
        "message": "Card network did not respond in 1800 ms",
        "retryable": True,
        "request_id": "req_8fK2mQx91LwZ",
        "upstream": {"service": "card-auth", "region": "ap-south-1", "attempts": 3, "last_status": None},
        "docs": "https://docs.payments.dev/errors#gateway_timeout",
    },
    "livemode": False,
}
RESPONSE_LINES = json.dumps(RESPONSE_JSON, indent=2).split("\n")
REQ_HEADERS = [
    (":method", "POST"), (":authority", "api.payments.dev"), (":path", "/v2/charges"),
    ("content-type", "application/json"), ("authorization", "Bearer sk_test_••••••••••••4f2a"),
    ("idempotency-key", "b2f1c9e0-6a1d-4e8b-9c33-0d7f5a2e11aa"), ("user-agent", "Mozilla/5.0 (X11; Linux x86_64) Chrome/131"),
    ("accept", "application/json"), ("accept-encoding", "gzip, deflate, br, zstd"), ("content-length", "186"),
    ("x-client-version", "web/4.18.2"),
]
RESP_HEADERS = [
    (":status", "502"), ("content-type", "application/json; charset=utf-8"), ("content-length", "412"),
    ("x-request-id", "req_8fK2mQx91LwZ"), ("retry-after", "2"), ("server", "envoy"), ("date", "Sat, 07 Nov 2026 08:32:13 GMT"),
]


# ---------------------------------------------------------------- toolbar
TOOLS = [("breakpoint", "Breakpoints", "2"), ("map", "Map Local", None), ("remote", "Map Remote", None),
         ("rewrite", "Rewrite", "3"), ("code", "Scripts", None), ("send", "Compose", None), ("diff", "Diff", None)]


def toolbar(parent, compact=False):
    tb = H(parent, 6, pad=[0, 12], h=44, sw="fill", fill="bg/raised", stroke="border/default", sides=[0, 0, 1, 0], name="Toolbar")
    logo(tb, 18, wordmark=not compact)
    # capture controls
    rec = H(tb, 6, pad=[0, 10], h=28, radius=6, fill="error/soft", name="Capture/recording")
    dot(rec, "error/default", 8)
    if not compact:
        T(rec, "Recording", 12, "Medium", "error/default")
    icon_btn(tb, "pause")
    icon_btn(tb, "trash")
    box(tb, None, w=1, h=20, fill="border/default", name="sep")
    for ic, label, n in TOOLS:
        if compact:
            icon_btn(tb, ic, active=n is not None)
            continue
        b = H(tb, 5, pad=[0, 8], h=28, radius=6, fill="accent/soft" if n else None, name="Tool/" + label)
        icon(b, ic, 14, "accent/text" if n else "text/secondary")
        T(b, label, 12, "Medium", "accent/text" if n else "text/secondary")
        if n:
            pill(b, n, "text/on-accent", "accent/default", size=10)
    spacer(tb)
    st = H(tb, 8, pad=[0, 10], h=28, radius=6, fill="bg/sunken", stroke="border/subtle", name="ProxyStatus")
    dot(st, "success/default", 7)
    M(st, ":9090", 11.5, "text/primary")
    if not compact:
        box(st, None, w=1, h=14, fill="border/default")
        icon(st, "shield-check", 13, "success/default")
        T(st, "CA trusted", 11.5, "Regular", "text/secondary")
    cp = H(tb, 6, pad=[0, 8], h=28, w=168 if not compact else 36, radius=6, fill="bg/sunken", stroke="border/default", name="CommandPalette")
    icon(cp, "command", 13, "text/tertiary")
    if not compact:
        T(cp, "Search or run a command…", 12, "Regular", "text/tertiary", sw="fill", trunc=True)
        kbd(cp, ["Ctrl", "K"])
    icon_btn(tb, "settings")
    wc = H(tb, 2, name="WindowControls")
    for ic in ("minus", "square", "x"):
        icon_btn(wc, ic, size=24)
    return tb


# ---------------------------------------------------------------- sidebar
def sidebar(parent, w=232):
    sb = V(parent, 0, w=w, sh="fill", fill="bg/panel", name="Sidebar", clip=True)
    top = H(sb, 6, pad=[10, 10, 6, 10], sw="fill")
    seg(top, ["App", "Domain", "Device"], 0, w=w - 54)
    icon_btn(top, "sidebar", size=26)
    sr = H(sb, 0, pad=[0, 10, 6, 10], sw="fill")
    text_input(sr, placeholder="Find source…", ic="search", h=26)
    body = V(sb, 1, pad=[0, 6], sw="fill", sh="fill", name="Sources")
    section_label(body, "Pinned", "2")
    tree_item(body, "POST /v2/sessions", ic="pin", ic_color="accent/text", mono=True, count="1")
    tree_item(body, "PATCH /v1/settings/…", ic="pin", ic_color="accent/text", mono=True, count="1")
    section_label(body, "Favourites", "3")
    for d, n in [("api.payments.dev", "86"), ("graphql.shop.dev", "41"), ("api.myapp.dev", "212")]:
        tree_item(body, d, ic="star", ic_color="warning/default", count=n, mono=True)
    section_label(body, "Sources", "2,484")
    tree_item(body, "Chrome", ic="app", expanded=True, count="1,204")
    tree_item(body, "api.payments.dev", depth=1, ic="lock", count="86", mono=True, selected=True)
    tree_item(body, "cdn.shopassets.net", depth=1, ic="lock", count="412", mono=True)
    tree_item(body, "api.myapp.dev", depth=1, ic="lock", count="212", mono=True)
    tree_item(body, "myapp.dev", depth=1, ic="unlock", ic_color="warning/default", count="18", mono=True)
    tree_item(body, "Slack", ic="app", expanded=False, count="388", hover=True)
    tree_item(body, "Music", ic="app", expanded=False, count="211")
    tree_item(body, "curl", ic="terminal", expanded=False, count="14")
    tree_item(body, "iPhone 16 Pro", ic="phone", expanded=True, count="302", right_icon="wifi")
    tree_item(body, "Shop", depth=1, ic="app", expanded=False, count="188")
    tree_item(body, "Weather", depth=1, ic="app", expanded=False, count="114")
    tree_item(body, "Pixel 9 · Android 16", ic="phone", expanded=False, count="96", right_icon="wifi")
    tree_item(body, "Bank app (SSL pinned)", ic="lock", ic_color="warning/default", expanded=False, count="9")
    ft = V(sb, 1, pad=[6, 6, 8, 6], sw="fill", stroke="border/subtle", sides=[1, 0, 0, 0], name="SavedSessions")
    section_label(ft, "Saved sessions", "4")
    tree_item(ft, "checkout-bug-1104.xws", ic="layers", count="Nov 4")
    tree_item(ft, "graphql-perf.har", ic="file", count="Oct 29")
    r = H(ft, 6, pad=[2, 10], sw="fill")
    icon(r, "plus", 12, "text/tertiary")
    T(r, "Open or import HAR, .chls, .proxymansession…", 11.5, "Regular", "text/tertiary", sw="fill", trunc=True)
    return sb


# ---------------------------------------------------------------- centre
QUERY = [("status", "key"), (":", "op"), ("5xx", "val"), (" ", "plain"), ("host", "key"), (":", "op"), ("*.payments.dev", "glob"),
         (" ", "plain"), ("method", "key"), (":", "op"), ("POST", "val"), (" ", "plain"), ("body", "key"), (":", "op"),
         ('"timeout"', "str")]
CHIPS = [("All", None), ("XHR", "1.4k"), ("JS", "212"), ("CSS", "38"), ("Img", "301"), ("Media", "12"),
         ("WS", "3"), ("gRPC", "27"), ("GraphQL", "86")]


def filter_bar(parent, compact=False, focus=False, tokens=QUERY):
    fb = V(parent, 0, sw="fill", fill="bg/panel", stroke="border/subtle", sides=[0, 0, 1, 0], name="FilterBar")
    r1 = H(fb, 6, pad=[8, 8, 6, 8], sw="fill")
    query_field(r1, tokens, focus=focus, count="3 of 2,484")
    icon_btn(r1, "star", size=28)
    icon_btn(r1, "history", size=28)
    r2 = H(fb, 4, pad=[0, 8, 8, 8], sw="fill", name="QuickChips", clip=True)
    chips = CHIPS[:6] if compact else CHIPS
    for i, (lb, n) in enumerate(chips):
        chip(r2, lb, on=lb == "XHR", count=n if lb != "All" else None)
    chip(r2, "Errors", on=False, count="17", ic="alert", tone="error/default")
    if not compact:
        spacer(r2)
        icon_btn(r2, "sliders", size=22)
    return fb


def request_table(parent, rows=ROWS, n=28, cols=None, selected=SELECTED, hover=HOVER, new_count=6):
    t = V(parent, 0, sw="fill", sh="fill", fill="bg/panel", name="RequestTable", clip=True)
    table_header(t, cols)
    for r in rows[:n]:
        st = "default"
        if r["idx"] == selected:
            st = "selected"
        elif r["idx"] == hover:
            st = "hover"
        elif r.get("pinned"):
            st = "pinned"
        table_row(t, r, st, cols)
    return t


# ---------------------------------------------------------------- inspector
def inspector_header(parent):
    hd = V(parent, 6, pad=[10, 12], sw="fill", fill="bg/raised", stroke="border/subtle", sides=[0, 0, 1, 0], name="InspectorHeader")
    r = H(hd, 8, sw="fill")
    method_badge(r, "POST", solid=True)
    M(r, "https://api.payments.dev/v2/charges", 12, "text/primary", sw="fill", trunc=True)
    for ic in ("pin", "repeat", "send", "copy", "more"):
        icon_btn(r, ic, size=24, active=ic == "pin" and False)
    m = H(hd, 10, sw="fill")
    status_badge(m, 502, "Bad Gateway")
    for ic, t in [("clock", "1.84 s"), ("upload", "186 B"), ("download", "412 B"), ("lock", "TLS 1.3 · h2")]:
        g = H(m, 4)
        icon(g, ic, 12, "text/tertiary")
        M(g, t, 11, "text/secondary")
    return hd


def timing_bar(parent):
    w = V(parent, 6, sw="fill", name="Timing")
    segs = [("DNS", 4, "status/3xx"), ("Connect", 12, "info/default"), ("TLS", 18, "syntax/keyword"),
            ("Waiting (TTFB)", 1790, "warning/default"), ("Download", 16, "success/default")]
    total = sum(s[1] for s in segs)
    for nm, ms, c in segs:
        r = H(w, 8, sw="fill")
        T(r, nm, 11.5, "Regular", "text/secondary", w=100)
        bar = H(r, 0, sw="fill", h=8, radius=2, fill="bg/sunken")
        box(bar, None, w=max(3, int(ms / total * 260)), h=8, radius=2, fill=c)
        M(r, "%d ms" % ms, 11, "text/primary", w=60, align="right")
    return w


def inspector(parent, w=None, sh="fill", layout="right", req_tab=0, resp_tab=2, body_mode=0, sw=None, h=None):
    ins = V(parent, 0, w=w, h=h, sw=sw, sh=sh, fill="bg/panel", name="Inspector", clip=True)
    inspector_header(ins)
    side = layout == "bottom"
    panes = H(ins, 0, sw="fill", sh="fill", cross="min") if side else V(ins, 0, sw="fill", sh="fill")
    # request
    rq = V(panes, 0, sw="fill", sh="fill" if side else None, h=None if side else 248, name="RequestPane", clip=True)
    tabs(rq, ["Headers", "Query", "Cookies", "Body", "Auth", "Timing"], req_tab, size="sm",
         counts={"Headers": 11, "Query": 0, "Cookies": 3},
         right=lambda r: (T(r, "REQUEST", 10, "Semi Bold", "text/tertiary", ls=0.6), icon_btn(r, "copy", size=22)), pad=[0, 6])
    kv_table(rq, REQ_HEADERS[:9] if not side else REQ_HEADERS[:8], key_w=128)
    split_handle(panes, vertical=side, hover=False)
    # response
    rs = V(panes, 0, sw="fill", sh="fill", name="ResponsePane", clip=True)
    tabs(rs, ["Headers", "Cookies", "Body", "Timing", "TLS"], resp_tab, size="sm", counts={"Headers": 7, "Cookies": 0},
         right=lambda r: (T(r, "RESPONSE", 10, "Semi Bold", "text/tertiary", ls=0.6),), pad=[0, 6])
    tl = H(rs, 6, pad=[6, 8], sw="fill", name="BodyToolbar")
    seg(tl, ["Pretty", "Raw", "Tree", "Hex", "Preview"], body_mode, size="sm")
    pill(tl, "JSON", "text/secondary", "bg/hover", size=10.5)
    spacer(tl)
    for ic in ("search", "wrap", "copy", "download"):
        icon_btn(tl, ic, size=22)
    wrap = V(rs, 0, pad=[0, 8, 8, 8], sw="fill", sh="fill")
    if body_mode == 0:
        code_block(wrap, RESPONSE_LINES, sh="fill", highlight=4)
    elif body_mode == 2:
        json_tree(wrap, JSON_TREE_ROWS, sel=3)
    return ins


JSON_TREE_ROWS = [
    (0, "error", "{7}", "object", True, "object"), (1, "type", '"upstream_error"', "string", None, "string"),
    (1, "code", '"gateway_timeout"', "string", None, "string"), (1, "message", '"Card network did not respond…"', "string", None, "string"),
    (1, "retryable", "true", "bool", None, "boolean"), (1, "request_id", '"req_8fK2mQx91LwZ"', "string", None, "string"),
    (1, "upstream", "{4}", "object", True, "object"), (2, "service", '"card-auth"', "string", None, "string"),
    (2, "region", '"ap-south-1"', "string", None, "string"), (2, "attempts", "3", "number", None, "number"),
    (2, "last_status", "null", "null", None, "null"), (1, "docs", '"https://docs.payments.dev/errors#…"', "string", None, "string"),
    (0, "livemode", "false", "bool", None, "boolean"),
]


def status_bar(parent, compact=False):
    sb = H(parent, 14, pad=[0, 12], h=24, sw="fill", fill="bg/raised", stroke="border/default", sides=[1, 0, 0, 0], name="StatusBar")
    g = H(sb, 6)
    dot(g, "success/default", 7)
    T(g, "Live", 11, "Medium", "success/default")
    M(g, "38 req/s", 11, "text/secondary")
    T(g, "·", 11, "Regular", "text/tertiary")
    M(g, "0.0.0.0:9090", 11, "text/secondary")
    items = [("2,484 requests · 48.2 MB", None), ("3 shown", None), ("Breakpoints 2 · 1 paused", "warning/default"),
             ("Rewrite 3 rules", None), ("Throttle off", None)]
    if compact:
        items = items[:3]
    for t, c in items:
        T(sb, t, 11, "Regular", c or "text/secondary")
    spacer(sb)
    if not compact:
        for keys, lab in [(["Ctrl", "L"], "Filter"), (["Space"], "Pause"), (["Ctrl", "K"], "Commands"), (["?"], "Shortcuts")]:
            h = H(sb, 5)
            kbd(h, keys, 10)
            T(h, lab, 11, "Regular", "text/tertiary")
    M(sb, "xwire 0.1.0", 10.5, "text/tertiary")
    return sb


# ---------------------------------------------------------------- whole screens
def workspace(x, y, mode="Dark", name="Workspace / Dark", overlays=None, body_mode=0, tokens=None):
    root = box(None, None, w=1440, h=900, fill="bg/app", name=name, x=x, y=y, clip=True)
    app = V(root, 0, w=1440, h=900, fill="bg/app", x=0, y=0, name="App")
    toolbar(app)
    body = H(app, 0, sw="fill", sh="fill", cross="min", name="Body")
    sidebar(body)
    split_handle(body)
    centre = V(body, 0, sw="fill", sh="fill", name="Centre", clip=True)
    filter_bar(centre, focus=bool(overlays), tokens=tokens or QUERY)
    request_table(centre)
    split_handle(body)
    inspector(body, w=470, body_mode=body_mode)
    status_bar(app)
    nw = H(root, 6, pad=[0, 12], h=26, radius=13, fill="accent/default", effects=SHADOW[2], name="NewRequestsPill", x=528, y=834)
    icon(nw, "download", 12, "text/on-accent", 2)
    T(nw, "6 new requests", 12, "Medium", "text/on-accent")
    kbd(nw, ["End"])
    if overlays:
        overlays(root)
    theme(root, mode)
    return root


def workspace_compact(x, y, mode="Dark"):
    """1024 × 700: icon rail, fewer columns, inspector docked at the bottom (request | response side by side)."""
    root = box(None, "v", 0, w=1024, h=700, fill="bg/app", name="Workspace / 1024 × 700 · " + mode, x=x, y=y, clip=True)
    toolbar(root, compact=True)
    body = H(root, 0, sw="fill", sh="fill", cross="min")
    rail = V(body, 4, w=44, sh="fill", pad=[8, 0], fill="bg/panel", cross="center", name="SidebarRail")
    for ic, on in [("app", True), ("globe", False), ("phone", False), ("pin", False), ("star", False), ("layers", False)]:
        icon_btn(rail, ic, active=on)
    split_handle(body)
    main = V(body, 0, sw="fill", sh="fill", clip=True)
    filter_bar(main, compact=True)
    cols = ["idx", "tls", "method", "status", "host", "path", "size", "dur"]
    request_table(main, n=9, cols=cols)
    split_handle(main, vertical=False, hover=True)
    inspector(main, sh="fixed", h=300, layout="bottom", sw="fill", body_mode=0)
    status_bar(root, compact=True)
    theme(root, mode)
    return root
