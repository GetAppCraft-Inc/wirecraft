"""Build the xwire design as a FigCraft command script.

    python3 main.py out.json
"""
import sys
from ds import *
from onboarding import *
import workspace as ws

GAP = 160


class Page:
    first = True

    def __init__(self, name):
        cmd("page.rename" if Page.first else "page.add", {"name": name})
        Page.first = False
        cmd("page.setBackground", {"color": "#050608"})


def overlays_interactions(root):
    """Context menu on the selected row, query autocomplete, toast, tooltip."""
    ac = V(root, 0, pad=4, w=430, fill="bg/overlay", radius=8, stroke="border/strong", effects=SHADOW[2], name="Autocomplete", x=241, y=82)
    hd = H(ac, 6, pad=[6, 8, 4, 8], sw="fill")
    M(hd, "status:", 11, "accent/text", wt="Bold")
    T(hd, "response status code or class", 11, "Regular", "text/tertiary")
    for i, (v, d, n) in enumerate([("5xx", "Server errors", "17"), ("502", "Bad Gateway", "6"), ("500", "Internal Server Error", "4"),
                                   ("503", "Service Unavailable", "7"), ("4xx", "Client errors", "41"), ("pending", "No response yet", "2")]):
        r = H(ac, 8, pad=[0, 8], h=26, sw="fill", radius=5, fill="accent/soft" if i == 0 else None)
        M(r, v, 12, "syntax/number" if v[0].isdigit() else "syntax/keyword", w=64)
        T(r, d, 12, "Regular", "text/secondary", sw="fill")
        M(r, n, 11, "text/tertiary")
    ft = H(ac, 12, pad=[6, 8, 4, 8], sw="fill", stroke="border/subtle", sides=[1, 0, 0, 0])
    for keys, lab in [(["↑", "↓"], "Navigate"), (["Tab"], "Complete"), (["↵"], "Apply"), (["Esc"], "Close")]:
        h = H(ft, 5)
        kbd(h, keys, 10)
        T(h, lab, 11, "Regular", "text/tertiary")
    menu(root, [
        (None, "POST /v2/charges", None, "header"),
        ("pin", "Pin", ["P"], None), ("star", "Add host to favourites", None, None), "-",
        ("copy", "Copy URL", ["Ctrl", "C"], None), ("terminal", "Copy as cURL", ["Ctrl", "⇧", "C"], "hover"),
        ("code", "Copy as…", None, "sub"), "-",
        ("repeat", "Repeat", ["R"], None), ("send", "Edit and repeat", ["E"], None), ("diff", "Compare with…", ["D"], None), "-",
        ("breakpoint", "Breakpoint on this endpoint", ["B"], None), ("map", "Map local…", None, None), ("remote", "Map remote…", None, None),
        ("rewrite", "New rewrite rule…", None, None), ("x", "Block host", None, None), "-",
        ("download", "Export selection as HAR", None, None), ("trash", "Delete", ["Del"], "danger"),
    ], w=256, x=560, y=275)
    sub = menu(root, [("terminal", "cURL", None, None), ("code", "fetch (JavaScript)", None, None), ("code", "Python requests", None, None),
                      ("code", "Go net/http", None, None), ("code", "Rust reqwest", None, None), ("code", "HTTPie", None, None)],
               w=190, x=812, y=410)
    toast(root, "success", "Copied as cURL", "POST https://api.payments.dev/v2/charges · 11 headers", "Undo", w=360, x=1060, y=824)
    tooltip(root, "Edit and repeat", ["E"], x=1260, y=82)


def main(out):
    define_tokens()
    # ------------------------------------------------------------ cover
    Page("Cover")
    cv = V(None, 24, pad=[96, 96], w=1600, h=960, fill="bg/app", name="Cover", x=0, y=0, clip=True)
    logo(cv, 48)
    box(cv, None, h=24, w=10)
    T(cv, "xwire — desktop UI", 60, "Bold", "text/primary", ls=-1.5)
    T(cv, "An open-source, native (Rust) HTTPS debugging proxy. Dense, keyboard-first, built to stay open all day.",
      22, "Regular", "text/secondary", w=1100, lh="140%")
    box(cv, None, h=24, w=10)
    g = H(cv, 48, cross="min")
    for colm in [["Design system — tokens, type, components", "Onboarding — welcome, root CA wizard, proxy & port"],
                 ["Main workspace — 1440 dark & light", "Workspace 1024 × 700 — bottom inspector", "Interactions — menu, autocomplete, toast"]]:
        v = V(g, 14, w=560)
        for p in colm:
            r = H(v, 12)
            dot(r, "accent/default", 8)
            T(r, p, 19, "Medium", "text/primary")
    box(cv, None, h=40, w=10)
    T(cv, "Dark first, light via variable modes · 1440 × 900 frames, checked at 1024 × 700 · Inter + monospace · v0.1 · Nov 2026",
      15, "Regular", "text/tertiary")
    theme(cv, "Dark")

    # ------------------------------------------------------------ design system
    Page("Design system")
    colour_board(0, 0)
    http_board(0, 1980)
    type_board(1880, 0)
    space_board(1880, 860)
    component_sheet(0, 2700, "Dark")
    component_sheet(1880, 2700, "Light")

    # ------------------------------------------------------------ onboarding
    Page("Onboarding & setup")
    X, Y = 1440 + GAP, 900 + GAP
    welcome(0, 0)
    welcome(X, 0, "Light")
    proxy_setup(2 * X, 0)
    ca_desktop(0, Y, "macOS")
    ca_desktop(X, Y, "Windows")
    ca_desktop(2 * X, Y, "Linux")
    ca_firefox(0, 2 * Y)
    ca_mobile(X, 2 * Y, "Android")
    ca_mobile(2 * X, 2 * Y, "iOS")
    ca_desktop(0, 3 * Y, "Linux", "Light")
    proxy_setup(X, 3 * Y, "Light")

    # ------------------------------------------------------------ workspace
    Page("Main workspace")
    ws.workspace(0, 0, "Dark", "Workspace / 1440 · Dark")
    ws.workspace(X, 0, "Light", "Workspace / 1440 · Light")
    ws.workspace(0, Y, "Dark", "Workspace / Interactions · Dark", overlays=overlays_interactions,
                 tokens=[("method", "key"), (":", "op"), ("POST", "val"), (" status", "key"), (":", "op"), ("5", "val")])
    ws.workspace(X, Y, "Dark", "Workspace / JSON tree body · Dark", body_mode=2)
    ws.workspace_compact(0, 2 * Y, "Dark")
    ws.workspace_compact(1024 + GAP, 2 * Y, "Light")

    finish_components()
    cmd("page.select", {"index": 0})
    print(save(out), "commands")


if __name__ == "__main__":
    main(sys.argv[1])
