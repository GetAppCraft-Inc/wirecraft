# xwire design

**This is the source of truth for the UI.** Build screens to match these designs. If a change departs from them, update the design first or in the same change.

| Path | What |
|---|---|
| `xwire.figcraft` | The design file. It has 4 pages: Cover, Design system, Onboarding & setup, and Main workspace |
| `screens/<page>/<frame>.png` | A 1× PNG of every frame, for reviewing and building without FigCraft |
| `generator/` | Python that generates the file as a FigCraft command script. The file is generated, not hand-drawn |

Open it in FigCraft with `flatpak run ai.storyteller.figcraft`, then File → Open.

## Foundations

- **Colour:** every fill and stroke is bound to a variable in `xwire/color`, which has a **Dark** and a **Light** mode. A Light frame is
  the same set of layers with `variable.setMode` switched, never a re-colour. The tokens are in `generator/lib.py` (`TOKENS`): `bg/*`,
  `border/*`, `text/*`, `accent/*` (periwinkle `#8B93FF` / `#4B52D6`), semantic colours, `method/*`, `status/1xx…5xx`, `syntax/*` and `row/*`.
- **Dimensions:** `xwire/dimension` holds `space/4…48` (4 px grid), `radius/xs…full`, and `size/row` (24), `row-compact` (20),
  `control` (28), `toolbar` (44) and `statusbar` (24).
- **Type:** Inter for the UI, sized 11–14 px for density, with larger sizes only for onboarding headings. URLs, headers, bodies and code
  use a monospace face. The app uses **JetBrains Mono**, but the file renders with DejaVu Sans Mono because FigCraft doesn't bundle
  JetBrains Mono. Text styles are under `xwire/…`.
- **Icons:** an original stroked 24 px set (`ICON_SRC` in `lib.py`). Nothing is taken from Proxyman, Charles or HTTP Toolkit.
- **Components:** the main components are on *Components · Dark*: Toolbar, Button/*, Segmented, Tabs, Chip/*, QueryField, TreeView,
  `TableRow` (variants Default, Hover, Selected, Pinned, Error, Paused, Pending), CodeBlock, JsonTree, HexView, Modal, Toast,
  ContextMenu, EmptyState, Kbd and Tooltip.

## Screens

| Frame | Notes |
|---|---|
| Onboarding / 1 Welcome (Dark, Light) | Value prop, 3 setup steps, live-traffic illustration |
| Onboarding / 2 Root CA · macOS, Windows, Linux (Dark, Light), Firefox, Android, iOS | Platform tabs; one-click install plus the manual command; certificate card; per-store trust status. Android and iOS add a QR code, the proxy address, adb/simulator setup and the Android network-security-config snippet |
| Onboarding / 3 Proxy & port (Dark, Light) | System proxy on/off and bypass list, ports (with a free-port check), listen interface, HTTPS interception scope, overall trust indicator |
| Workspace / 1440 · Dark, Light | Sidebar (App/Domain/Device, pinned, favourites, saved sessions), query filter bar and quick chips, virtualized table with all columns, right inspector split into request and response, status bar |
| Workspace / Interactions · Dark | Query autocomplete, row context menu with the *Copy as…* submenu, toast, tooltip |
| Workspace / JSON tree body · Dark | Response body in Tree mode |
| Workspace / 1024 × 700 · Dark, Light | Icon rail, fewer columns, inspector docked at the bottom with request and response side by side |

## Changing the design

Edit the generator, not the file. Primitives and tokens are in `lib.py`, components in `ui.py`, and screens in `workspace.py`,
`onboarding.py` and `ds.py`. The page layout is in `main.py`. Then rebuild. FigCraft's Flatpak sandbox can only reach `~/Documents`,
so build there and copy the results back:

```bash
W=~/Documents/xwire-design && mkdir -p $W
F="flatpak run --command=figcraft-cli ai.storyteller.figcraft"
python3 design/generator/main.py $W/xwire.json
$F run --automation-write-root $W --out $W/xwire.figcraft $W/xwire.json
for p in "Cover:cover" "Design system:design-system" "Onboarding & setup:onboarding" "Main workspace:workspace"; do
  mkdir -p $W/screens/${p##*:}; $F export --automation-write-root $W --page "${p%%:*}" -o $W/screens/${p##*:} $W/xwire.figcraft
done
cp $W/xwire.figcraft design/   # then copy the PNGs into design/screens/ under kebab-case names
```

Watch out for one quirk: text layers drop trailing spaces. `lib.T()` turns them into no-break spaces, so build spaced runs (code
tokens, the query field) through `T`/`M`.

## Not designed yet

The pasted brief was cut off partway through the inspector section. These are still to come: inspector tabs beyond
Headers/Query/Cookies/Body (Image/HTML preview bodies, Timing and TLS detail), and the tools behind the toolbar: Breakpoints,
Map Local/Remote, Rewrite, Scripts, Compose, Diff, Throttle, WebSocket/gRPC message views, sessions/HAR import, and Settings.
