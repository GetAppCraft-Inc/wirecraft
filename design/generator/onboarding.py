"""Onboarding & setup: welcome, root CA wizard (6 platforms), proxy & port."""
from ui import *

LAN = "192.168.1.24"


def window(x, y, name, mode="Dark", w=1440, h=900):
    if mode != "Dark":
        name += " · " + mode
    root = V(None, 0, w=w, h=h, fill="bg/app", name=name, x=x, y=y, clip=True)
    tb = H(root, 8, pad=[0, 12], h=40, sw="fill", fill="bg/raised", stroke="border/default", sides=[0, 0, 1, 0], name="TitleBar")
    logo(tb, 16)
    T(tb, "Setup", 12, "Regular", "text/tertiary")
    spacer(tb)
    for ic in ("minus", "square", "x"):
        icon_btn(tb, ic, size=24)
    return root


def stepper(parent, active):
    s = H(parent, 10, name="Stepper")
    for i, lab in enumerate(["Root certificate", "Proxy & port", "Ready"]):
        done, on = i < active, i == active
        c = H(s, 0, w=20, h=20, radius=10, main="center",
              fill="accent/default" if on else ("success/soft" if done else "bg/raised"),
              stroke=None if on else ("success/default" if done else "border/default"))
        if done:
            icon(c, "check", 11, "success/default", 2.2)
        else:
            T(c, str(i + 1), 11, "Semi Bold", "text/on-accent" if on else "text/tertiary")
        T(s, lab, 12.5, "Medium" if on else "Regular", "text/primary" if on or done else "text/tertiary")
        if i < 2:
            box(s, None, w=40, h=1, fill="success/default" if done else "border/default")
    return s


def card(parent, gap=12, pad=16, sw="fill", **kw):
    return V(parent, gap, pad=pad, fill="bg/panel", radius=10, stroke="border/default", sw=sw, **kw)


def step(parent, n, title, body=None, done=False, extra=None):
    r = H(parent, 12, sw="fill", cross="min", name="Step %d" % n)
    c = H(r, 0, w=22, h=22, radius=11, main="center", fill="success/soft" if done else "bg/raised",
          stroke="success/default" if done else "border/default")
    if done:
        icon(c, "check", 12, "success/default", 2.2)
    else:
        T(c, str(n), 11, "Semi Bold", "text/secondary")
    col_ = V(r, 6, sw="fill")
    T(col_, title, 13, "Medium", "text/primary", sw="fill")
    if body:
        T(col_, body, 12.5, "Regular", "text/secondary", sw="fill", lh="150%")
    if extra:
        extra(col_)
    return r


def banner(parent, tone, title, body=None, action=None):
    b = H(parent, 10, pad=[10, 12], sw="fill", radius=8, fill=tone + "/soft", stroke=tone + "/default", cross="min", name="Banner/" + tone)
    icon(b, {"success": "shield-check", "warning": "alert", "error": "alert", "info": "info"}[tone], 16, tone + "/default")
    t = V(b, 2, sw="fill")
    T(t, title, 12.5, "Medium", "text/primary", sw="fill")
    if body:
        T(t, body, 12, "Regular", "text/secondary", sw="fill", lh="150%")
    if action:
        btn(b, action, "secondary", size="sm")
    return b


# ---------------------------------------------------------------- 1. welcome
def welcome(x, y, mode="Dark"):
    root = window(x, y, "Onboarding / 1 Welcome", mode)
    body = H(root, 64, pad=[0, 112], sw="fill", sh="fill", cross="center", name="Welcome")
    left = V(body, 20, w=540)
    pill(left, "OPEN SOURCE  ·  NATIVE RUST  ·  MIT", "accent/text", "accent/soft", size=10.5)
    T(left, "See every request your apps make.", 34, "Bold", "text/primary", w=520, lh="118%", ls=-0.6)
    T(left, "xwire is a native HTTPS debugging proxy. Inspect, break, rewrite and replay HTTP/1.1, HTTP/2, "
            "WebSocket and gRPC traffic from desktop apps, browsers and phones.", 14, "Regular", "text/secondary", w=520, lh="155%")
    steps = V(left, 0, sw="fill", radius=10, stroke="border/default", fill="bg/panel", clip=True)
    for i, (ic, t, d, k) in enumerate([
        ("shield", "Install the xwire root certificate", "So HTTPS can be decrypted on this machine and your devices.", "2 min"),
        ("plug", "Pick a port and turn on the system proxy", "xwire listens on :9090 and restores your settings on quit.", "30 s"),
        ("phone", "Connect phones and simulators", "Optional. Scan a QR code — no cables or USB debugging.", "optional")]):
        r = H(steps, 12, pad=[12, 14], sw="fill", stroke="border/subtle" if i < 2 else None, sides=[0, 0, 1, 0] if i < 2 else None)
        c = H(r, 0, w=32, h=32, radius=8, fill="bg/raised", stroke="border/default", main="center")
        icon(c, ic, 16, "accent/text")
        tx = V(r, 2, sw="fill")
        T(tx, t, 13, "Medium", "text/primary")
        T(tx, d, 12, "Regular", "text/secondary", sw="fill")
        T(r, k, 11, "Regular", "text/tertiary")
    act = H(left, 10)
    btn(act, "Set up HTTPS interception", "primary", size="lg", keys=["↵"])
    btn(act, "Skip for now", "ghost", size="lg")
    meta = H(left, 14)
    for t in ("v0.1.0", "Telemetry off", "Imports .har  .chls  .proxymansession"):
        T(meta, t, 11.5, "Regular", "text/tertiary")
    # illustration: devices → xwire → internet, with a live request stream
    art = V(body, 14, w=560, pad=24, radius=14, fill="bg/panel", stroke="border/default", name="Illustration")
    flow = H(art, 0, sw="fill", cross="center", main="spaceBetween")
    for ic, lab, sub, on in [("monitor", "This computer", "Chrome · curl", False), (None, None, None, None),
                             ("logo", "xwire", "0.0.0.0:9090", True), (None, None, None, None),
                             ("globe", "Internet", "TLS 1.3 · h2", False)]:
        if ic is None:
            ln = H(flow, 0, w=56, h=2, fill="accent/default", name="wire")
            continue
        n = V(flow, 6, pad=[12, 10], w=136, radius=10, cross="center",
              fill="accent/soft" if on else "bg/raised", stroke="accent/default" if on else "border/default")
        if ic == "logo":
            logo(n, 22, wordmark=False)
        else:
            icon(n, ic, 20, "text/secondary")
        T(n, lab, 12.5, "Semi Bold", "text/primary")
        M(n, sub, 10.5, "text/tertiary")
    ph = H(art, 8, sw="fill", main="min")
    for lab in ("iPhone 16 Pro", "Pixel 9", "iOS Simulator"):
        p = H(ph, 6, pad=[4, 8], radius=6, fill="bg/raised", stroke="border/subtle")
        icon(p, "phone", 12, "text/secondary")
        T(p, lab, 11.5, "Regular", "text/secondary")
    stream = V(art, 0, sw="fill", radius=8, stroke="border/subtle", clip=True, fill="bg/sunken")
    for m, st, host, path, dur in [("GET", 200, "api.myapp.dev", "/v1/feed", "210 ms"), ("POST", 201, "api.myapp.dev", "/v1/orders", "180 ms"),
                                   ("GET", 101, "rt.slack.dev", "/websocket/v3", "live"), ("POST", 502, "api.payments.dev", "/v2/charges", "1.84 s"),
                                   ("POST", 200, "grpc.maps.dev", "/maps.v1.Routes/Compute", "96 ms"), ("GET", 304, "fonts.gstatic.com", "/s/inter/v13", "18 ms")]:
        r = H(stream, 10, pad=[0, 12], h=28, sw="fill", stroke="border/subtle", sides=[0, 0, 1, 0],
              fill="row/error" if st == 502 else None)
        method_badge(r, m)
        M(r, str(st), 11.5, status_var(st), w=30)
        M(r, host + path, 11.5, "error/default" if st == 502 else "text/secondary", sw="fill", trunc=True)
        M(r, dur, 11, "warning/default" if dur.endswith(" s") else "text/tertiary")
    T(art, "Everything stays on this machine. xwire never uploads captured traffic.", 11.5, "Regular", "text/tertiary")
    theme(root, mode)
    return root


# ---------------------------------------------------------------- 2. root CA wizard
PLATFORMS = ["macOS", "Windows", "Linux", "Firefox", "Android", "iOS"]
PLAT_ICON = {"macOS": "monitor", "Windows": "monitor", "Linux": "terminal", "Firefox": "globe", "Android": "phone", "iOS": "phone"}


def trust_list(parent, states):
    c = card(parent, 8, name="TrustStatus")
    r = H(c, 6, sw="fill")
    T(r, "Trust status", 13, "Semi Bold", "text/primary", sw="fill")
    btn(r, "Re-check", "ghost", "repeat", size="sm")
    for store, st, note in states:
        row = H(c, 8, sw="fill", h=26)
        tone = {"ok": "success/default", "warn": "warning/default", "off": "text/tertiary", "wait": "info/default"}[st]
        icon(row, {"ok": "shield-check", "warn": "alert", "off": "minus", "wait": "clock"}[st], 14, tone)
        T(row, store, 12.5, "Regular", "text/primary", sw="fill")
        T(row, note, 11.5, "Medium", tone)
    return c


def cert_card(parent):
    c = card(parent, 10, name="CertificateCard")
    r = H(c, 10, sw="fill")
    b = H(r, 0, w=32, h=32, radius=8, fill="accent/soft", main="center")
    icon(b, "shield", 16, "accent/text")
    t = V(r, 2, sw="fill")
    T(t, "xwire Root CA", 13, "Semi Bold", "text/primary")
    T(t, "Generated on this machine · never shared", 11.5, "Regular", "text/tertiary")
    for k, v in [("Common name", "xwire Root CA · tanmay-cachyos"), ("Key", "ECDSA P-256"), ("Valid", "07 Nov 2026 → 07 Nov 2027"),
                 ("SHA-256", "4F:2A:91:C3:0E:7B:…:D1:88")]:
        row = H(c, 8, sw="fill")
        T(row, k, 11.5, "Regular", "text/tertiary", w=92)
        M(row, v, 11, "text/primary", sw="fill", trunc=True)
    a = H(c, 6, sw="fill", wrap=True)
    for f in (".pem", ".crt", ".der", ".p12"):
        btn(a, f, "secondary", "download" if f == ".pem" else None, size="sm")
    hr(c)
    rr = H(c, 6, sw="fill")
    btn(rr, "Regenerate…", "danger", "repeat", size="sm")
    spacer(rr)
    T(rr, "~/.local/share/xwire/ca", 11, "Regular", "text/tertiary")
    return c


DESKTOP = {
    "macOS": dict(
        title="Trust the certificate in the System keychain",
        auto="Install automatically", auto_note="Asks for your password once. xwire adds the CA and sets “Always Trust”.",
        manual=["sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain ~/Library/Application\\ Support/xwire/ca/xwire-root.pem"],
        verify="Open https://xwire.check in Safari — a green page means HTTPS interception works.",
        trust=[("System keychain", "warn", "Not trusted"), ("Safari / Chrome", "warn", "Uses system"), ("Firefox", "off", "Separate step"),
               ("Java (cacerts)", "off", "Optional"), ("iOS Simulator", "off", "Optional")],
        banner=("warning", "HTTPS traffic can’t be decrypted yet", "Requests will show as encrypted CONNECT tunnels until the CA is trusted."),
    ),
    "Windows": dict(
        title="Add the certificate to Trusted Root Certification Authorities",
        auto="Install automatically", auto_note="Windows shows a security prompt — choose Yes. No restart needed.",
        manual=['certutil -addstore -f "ROOT" "%APPDATA%\\xwire\\ca\\xwire-root.crt"'],
        verify="Run the check below or open https://xwire.check in Edge.",
        trust=[("Windows root store", "warn", "Not trusted"), ("Edge / Chrome", "warn", "Uses system"), ("Firefox", "off", "Separate step"),
               ("WSL distributions", "off", "Optional")],
        banner=("warning", "HTTPS traffic can’t be decrypted yet", "Requests will show as encrypted CONNECT tunnels until the CA is trusted."),
    ),
    "Linux": dict(
        title="Add the certificate to the system trust store",
        auto="Install with pkexec", auto_note="Detected CachyOS (Arch). xwire runs the trust anchor command for you.",
        manual=["sudo trust anchor --store ~/.local/share/xwire/ca/xwire-root.crt"],
        verify="Verified: curl https://xwire.check returned 200 through the proxy.",
        trust=[("p11-kit (system)", "ok", "Trusted"), ("Chrome / Chromium (NSS db)", "ok", "Trusted"), ("Firefox", "warn", "Not installed"),
               ("Java (cacerts)", "off", "Optional"), ("Node.js", "warn", "Set NODE_EXTRA_CA_CERTS")],
        banner=("success", "Root certificate trusted", "HTTPS from curl, Chrome and most native apps on this machine can now be decrypted."),
        distro=True,
    ),
}


def wizard_shell(x, y, name, platform, mode="Dark", step_=0):
    root = window(x, y, name, mode)
    hd = H(root, 16, pad=[18, 32], sw="fill", name="WizardHeader")
    V_ = V(hd, 4, sw="fill")
    T(V_, "Install the xwire root certificate", 18, "Semi Bold", "text/primary")
    T(V_, "xwire signs certificates on the fly with a CA only this machine holds. Trust it once per device.", 12.5, "Regular", "text/secondary")
    stepper(hd, step_)
    body = H(root, 16, pad=[0, 32], sw="fill", sh="fill", cross="min", name="WizardBody")
    main = V(body, 0, sw="fill", sh="fill", radius=10, stroke="border/default", fill="bg/panel", clip=True, name="PlatformCard")
    tabs(main, PLATFORMS, PLATFORMS.index(platform), pad=[0, 12])
    content = V(main, 16, pad=20, sw="fill", sh="fill", name="Content")
    side = V(body, 16, w=360, name="Side")
    ft = H(root, 10, pad=[14, 32], sw="fill", stroke="border/default", sides=[1, 0, 0, 0], fill="bg/raised", name="WizardFooter")
    btn(ft, "Back", "ghost", "chevron-left")
    spacer(ft)
    T(ft, "You can reopen this from Settings → Certificates.", 11.5, "Regular", "text/tertiary")
    btn(ft, "Skip", "secondary")
    btn(ft, "Continue", "primary", keys=["↵"])
    return root, content, side


def ca_desktop(x, y, platform, mode="Dark"):
    d = DESKTOP[platform]
    root, content, side = wizard_shell(x, y, "Onboarding / 2 Root CA · " + platform, platform, mode)
    banner(content, *d["banner"])
    if d.get("distro"):
        r = H(content, 10)
        T(r, "Distribution", 12, "Medium", "text/secondary")
        seg(r, ["Debian / Ubuntu", "Fedora / RHEL", "Arch / CachyOS", "openSUSE"], 2)
    done = platform == "Linux"
    step(content, 1, d["title"], d["auto_note"], done=done,
         extra=lambda c: btn(H(c, 8), "Trusted" if done else d["auto"], "secondary" if done else "primary", "check" if done else "shield"))
    step(content, 2, "Or run it yourself", "Same result, if you’d rather not grant xwire admin rights.",
         extra=lambda c: shell_block(c, d["manual"]))
    step(content, 3, "Verify", d["verify"], done=done,
         extra=lambda c: btn(H(c, 8), "Run check", "secondary", "repeat", size="sm"))
    cert_card(side)
    trust_list(side, d["trust"])
    theme(root, mode)
    return root


def ca_firefox(x, y, mode="Dark"):
    root, content, side = wizard_shell(x, y, "Onboarding / 2 Root CA · Firefox", "Firefox", mode)
    banner(content, "info", "Firefox keeps its own certificate store", "xwire found 2 profiles. Pick how to make Firefox trust the CA.")
    opts = V(content, 8, sw="fill")
    for on, t, dsc in [(True, "Use the system trust store (recommended)", "Sets security.enterprise_roots.enabled = true in each profile. Survives CA regeneration."),
                       (False, "Import into each profile’s NSS database", "Uses certutil -d sql:<profile>. Works on locked-down installs.")]:
        r = H(opts, 10, pad=12, sw="fill", radius=8, stroke="accent/default" if on else "border/default", fill="accent/soft" if on else None, cross="min")
        radio(r, on)
        tx = V(r, 3, sw="fill")
        T(tx, t, 13, "Medium", "text/primary")
        T(tx, dsc, 12, "Regular", "text/secondary", sw="fill")
    T(content, "Profiles", 12, "Semi Bold", "text/secondary")
    pl = V(content, 0, sw="fill", radius=8, stroke="border/default", clip=True)
    for on, nm, path, st in [(True, "default-release", "~/.mozilla/firefox/k2x9w1.default-release", "Firefox 134"),
                             (True, "dev-edition", "~/.mozilla/firefox/7qh3a0.dev-edition-default", "Developer Edition 135"),
                             (False, "Flatpak · default", "~/.var/app/org.mozilla.firefox/…/default", "Not running")]:
        r = H(pl, 10, pad=[0, 12], h=36, sw="fill", stroke="border/subtle", sides=[0, 0, 1, 0])
        checkbox(r, on)
        T(r, nm, 12.5, "Medium", "text/primary", w=150)
        M(r, path, 11, "text/tertiary", sw="fill", trunc=True)
        T(r, st, 11.5, "Regular", "text/secondary")
    r = H(content, 10)
    btn(r, "Apply to 2 profiles", "primary", "check")
    T(r, "Firefox must be restarted. xwire will ask before closing it.", 12, "Regular", "text/tertiary")
    cert_card(side)
    trust_list(side, [("p11-kit (system)", "ok", "Trusted"), ("Firefox · default-release", "warn", "Pending"),
                      ("Firefox · dev-edition", "warn", "Pending"), ("Java (cacerts)", "off", "Optional")])
    theme(root, mode)
    return root


def ca_mobile(x, y, platform, mode="Dark"):
    root, content, side = wizard_shell(x, y, "Onboarding / 2 Root CA · " + platform, platform, mode)
    top = H(content, 24, sw="fill", cross="min")
    qcol = V(top, 10, w=200, cross="center", name="QR")
    qr(qcol, seed=platform, n=29, mod=5)
    M(qcol, "http://xwire.ca/%s" % ("android" if platform == "Android" else "ios"), 11, "accent/text")
    T(qcol, "Scan with the camera app", 11.5, "Regular", "text/tertiary")
    steps = V(top, 14, sw="fill")
    proxy = H(steps, 12, pad=[10, 12], sw="fill", radius=8, fill="bg/sunken", stroke="border/default")
    icon(proxy, "wifi", 16, "accent/text")
    t = V(proxy, 2, sw="fill")
    T(t, "Wi-Fi proxy · Manual", 11.5, "Regular", "text/tertiary")
    M(t, "%s : 9090" % LAN, 16, "text/primary", wt="Bold")
    btn(proxy, None, "ghost", "copy")
    if platform == "Android":
        step(steps, 1, "Join the same Wi-Fi and set the proxy", "Settings → Network & internet → Internet → (your network) → Proxy → Manual.")
        step(steps, 2, "Scan the QR code and install the certificate", "Settings → Security & privacy → More → Encryption & credentials → Install a certificate → CA certificate.")
        step(steps, 3, "Let your debug app trust user CAs", "Android 7+ apps ignore user CAs unless their network security config allows it.",
             extra=lambda c: code_block(c, [
                 [("<network-security-config>", "syntax/keyword")],
                 [("  <debug-overrides>", "syntax/keyword")],
                 [("    <trust-anchors>", "syntax/keyword")],
                 [("      <certificates ", "syntax/keyword"), ("src", "syntax/key"), ("=", "syntax/punct"), ('"user"', "syntax/string"), (" />", "syntax/keyword")],
                 [("    </trust-anchors>", "syntax/keyword")],
                 [("  </debug-overrides>", "syntax/keyword")],
                 [("</network-security-config>", "syntax/keyword")]], size=11))
        em = H(content, 10, pad=12, sw="fill", radius=8, stroke="border/default", fill="bg/raised")
        icon(em, "cpu", 16, "text/secondary")
        tx = V(em, 2, sw="fill")
        T(tx, "Emulator detected · Pixel_9_API_36 (emulator-5554)", 12.5, "Medium", "text/primary")
        T(tx, "xwire can push the CA as a system certificate and set the proxy over adb.", 12, "Regular", "text/secondary")
        btn(em, "Set up via adb", "secondary", "terminal")
        status = [("Pixel 9 · Android 16", "wait", "Waiting for device…"), ("Pixel_9_API_36 (emulator)", "off", "Not set up"),
                  ("User CA installed", "off", "—"), ("Traffic seen", "off", "—")]
    else:
        step(steps, 1, "Set the Wi-Fi proxy on your iPhone or iPad", "Settings → Wi-Fi → ⓘ next to your network → Configure Proxy → Manual.", done=True)
        step(steps, 2, "Scan the QR code to download the profile", "Safari opens xwire.ca and asks to download a configuration profile → Allow.", done=True)
        step(steps, 3, "Install the profile", "Settings → General → VPN & Device Management → xwire Root CA → Install.", done=True)
        step(steps, 4, "Turn on full trust", "Settings → General → About → Certificate Trust Settings → enable xwire Root CA. iOS needs this extra step.")
        sim = H(content, 10, pad=12, sw="fill", radius=8, stroke="border/default", fill="bg/raised")
        icon(sim, "cpu", 16, "text/secondary")
        tx = V(sim, 2, sw="fill")
        T(tx, "2 booted simulators · iPhone 16 Pro (iOS 26.1), iPad Air (iOS 26.1)", 12.5, "Medium", "text/primary")
        T(tx, "Requires macOS with Xcode · runs xcrun simctl keychain add-root-cert.", 12, "Regular", "text/secondary")
        btn(sim, "Install into simulators", "secondary", "download")
        status = [("iPhone 16 Pro · Tanmay’s", "ok", "Connected"), ("Profile installed", "ok", "Yes"),
                  ("Full trust enabled", "warn", "Pending"), ("HTTPS decrypted", "off", "—")]
    banner(content, "info", "Remove the proxy when you’re done",
           "Phones keep the manual proxy after xwire quits. xwire shows a reminder for devices it has seen.")
    cert_card(side)
    trust_list(side, status)
    theme(root, mode)
    return root


# ---------------------------------------------------------------- 3. proxy & port
def proxy_setup(x, y, mode="Dark"):
    root = window(x, y, "Onboarding / 3 Proxy & port", mode)
    hd = H(root, 16, pad=[18, 32], sw="fill")
    v = V(hd, 4, sw="fill")
    T(v, "Proxy & port", 18, "Semi Bold", "text/primary")
    T(v, "How apps reach xwire. Change any of this later in Settings → Proxy.", 12.5, "Regular", "text/secondary")
    stepper(hd, 1)
    body = H(root, 16, pad=[0, 32], sw="fill", sh="fill", cross="min")
    left = V(body, 16, sw="fill")
    # system proxy
    c = card(left, 12)
    r = H(c, 12, sw="fill")
    b = H(r, 0, w=32, h=32, radius=8, fill="accent/soft", main="center")
    icon(b, "plug", 16, "accent/text")
    t = V(r, 2, sw="fill")
    T(t, "System proxy", 13.5, "Semi Bold", "text/primary")
    T(t, "Routes HTTP and HTTPS from every app that honours the OS proxy. Restored on quit or crash.", 12, "Regular", "text/secondary", sw="fill")
    pill(r, "On", "success/default", "success/soft", dot_=True)
    toggle(r, True, 1.1)
    hr(c)
    f = H(c, 12, sw="fill")
    T(f, "Bypass", 12, "Medium", "text/secondary", w=110)
    text_input(f, "localhost, 127.0.0.1, ::1, *.local, 169.254/16", mono=True)
    f = H(c, 12, sw="fill")
    T(f, "Applies to", 12, "Medium", "text/secondary", w=110)
    seg(f, ["GNOME (gsettings)", "KDE", "Environment vars"], 0)
    T(f, "Also export HTTPS_PROXY for new terminals", 12, "Regular", "text/tertiary", sw="fill")
    toggle(f, False)
    # listener
    c = card(left, 12)
    T(c, "Listener", 13.5, "Semi Bold", "text/primary")
    g = H(c, 16, sw="fill", cross="min")
    pc = V(g, 6, w=180)
    T(pc, "HTTP / HTTPS port", 12, "Medium", "text/secondary")
    text_input(pc, "9090", mono=True, right="✓ free", focus=True)
    T(pc, "8080 is used by java (pid 4121)", 11, "Regular", "text/tertiary")
    sc = V(g, 6, w=180)
    rr = H(sc, 6, sw="fill")
    T(rr, "SOCKS5 port", 12, "Medium", "text/secondary", sw="fill")
    toggle(rr, False, 0.85)
    text_input(sc, "8889", mono=True)
    lc = V(g, 6, sw="fill")
    T(lc, "Listen on", 12, "Medium", "text/secondary")
    seg(lc, ["This computer only", "All interfaces"], 1, icons=["monitor", "wifi"])
    T(lc, "Needed for phones and other machines · reachable at %s:9090" % LAN, 11, "Regular", "text/tertiary")
    # interception
    c = card(left, 12)
    T(c, "HTTPS interception", 13.5, "Semi Bold", "text/primary")
    opts = H(c, 10, sw="fill")
    for on, ttl, dsc in [(True, "All hosts", "Decrypt everything except the SSL-pinning bypass list."),
                         (False, "Only listed hosts", "Tunnel everything else untouched."),
                         (False, "Off", "Inspect plain HTTP only.")]:
        o = V(opts, 4, pad=12, sw="fill", radius=8, stroke="accent/default" if on else "border/default", fill="accent/soft" if on else None)
        rr = H(o, 8)
        radio(rr, on)
        T(rr, ttl, 12.5, "Medium", "text/primary")
        T(o, dsc, 11.5, "Regular", "text/secondary", sw="fill")
    rr = H(c, 8, sw="fill")
    T(rr, "Never decrypt", 12, "Medium", "text/secondary", w=110)
    for h_ in ("*.apple.com", "accounts.vendor.dev", "*.bank.example"):
        pill(rr, h_, "text/secondary", "bg/hover", mono=True)
    btn(rr, "Add host", "ghost", "plus", size="sm")
    # right column: overall status
    side = V(body, 16, w=360)
    s = card(side, 10, name="TrustIndicator")
    T(s, "Ready to capture", 13.5, "Semi Bold", "text/primary")
    for ic, lab, val, tone in [("shield-check", "Root CA", "Trusted by system", "success/default"),
                               ("plug", "System proxy", "On · 127.0.0.1:9090", "success/default"),
                               ("wifi", "LAN listener", "%s:9090" % LAN, "success/default"),
                               ("globe", "Firefox", "Not trusted", "warning/default"),
                               ("phone", "Devices", "1 connected", "info/default")]:
        r = H(s, 8, sw="fill", h=26)
        icon(r, ic, 14, tone)
        T(r, lab, 12.5, "Regular", "text/primary", sw="fill")
        T(r, val, 11.5, "Medium", tone)
    btn(s, "Fix Firefox", "secondary", "globe", size="sm")
    tip = card(side, 8)
    T(tip, "Keyboard first", 12.5, "Semi Bold", "text/primary")
    for keys, lab in [(["Ctrl", "R"], "Start / stop recording"), (["Space"], "Pause the live table"), (["Ctrl", "L"], "Focus filter"),
                      (["Ctrl", "K"], "Command palette"), (["?"], "All shortcuts")]:
        r = H(tip, 8, sw="fill")
        T(r, lab, 12, "Regular", "text/secondary", sw="fill")
        kbd(r, keys)
    ft = H(root, 10, pad=[14, 32], sw="fill", stroke="border/default", sides=[1, 0, 0, 0], fill="bg/raised")
    btn(ft, "Back", "ghost", "chevron-left")
    spacer(ft)
    btn(ft, "Finish & start capturing", "primary", "record", keys=["↵"])
    theme(root, mode)
    return root
