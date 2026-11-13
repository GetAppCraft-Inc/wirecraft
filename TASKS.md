# xwire build — progress

Stack (user-chosen 2026-11-11): **Tauri 2 + Svelte 5/TS UI** over a **Rust MITM core**. Scope for v1: real proxy +
3-pane workspace + onboarding, matching `design/` (source of truth; tokens in `design/generator/lib.py`).

## Done
- Cargo workspace (`Cargo.toml`: members `crates/xwire-core`, `src-tauri`).
- `crates/xwire-core` — builds, unit tests pass, smoke-tested with curl:
  - `ca.rs` CA gen/load (ECDSA P-256, `~/.local/share/xwire/ca/xwire-root.{pem,crt,der,key}`), per-host leaf certs cached, SNI resolver.
  - `proxy.rs` HTTP/1.1 proxy, CONNECT → TLS intercept (h2+h1 to client, auto builder) or blind tunnel; upstream
    hyper-util client (h1+h2, native roots) + h1-only client for WebSocket upgrades; Host header stripped (h2 safe);
    TLS-rejection/pinning recorded as CONNECT error flow; `xwire.check` / `xwire.cert` special hosts; landing page
    on direct hits; `replay(id)`; intercept modes All/Listed/Off + never-decrypt globs.
  - `store.rs` flows + broadcast events (Upsert/Removed/Cleared), recording toggle, cap.
  - `body.rs` TeeBody (captures response while streaming; finishes on is_end_stream), gzip/deflate/br/zstd decode, type classify.
  - `procinfo.rs` Linux /proc client-process lookup (Client column) + User-Agent fallback.
  - `examples/headless.rs` — `cargo run --example headless -- 127.0.0.1:9090` (env `XWIRE_CA_DIR`).
- `src-tauri/`: Cargo.toml, build.rs, tauri.conf.json (decorations off, 1440×900, min 1024×700, devUrl :1420),
  capabilities/default.json, icons (generated from icons/xwire.svg), `src/settings.rs`, `src/system.rs`
  (os info, trust status/install for p11-kit/NSS/Firefox/macOS/Windows, GNOME/KDE system proxy, port check, LAN ip).
- npm: package.json with vite 8, svelte 5, @sveltejs/vite-plugin-svelte 7, typescript 6, @tauri-apps/cli+api 2,
  plugin-dialog, plugin-clipboard-manager, @fontsource-variable/inter, @fontsource/jetbrains-mono.

## Resume here (next steps)
1. `src-tauri/src/lib.rs` + `main.rs`: AppState (proxy, store, ca, settings); start proxy on settings.port;
   forward store events batched (~60 ms) as `flows` event; commands: app_state, list_flows, flow_detail
   (headers, timing, tls, bodies decoded via `xwire_core::proxy::decoded_body`, base64 for binary), set_recording,
   clear_flows, remove_flows, replay, restart_proxy(port, listen_all), set_intercept, trust_status, install_ca_system,
   install_nss(firefox), regenerate_ca, manual_commands, system_proxy get/set (restore on quit), check_port, lan_ip,
   export_har, favourites. Register plugins dialog/clipboard/opener. Then `cargo build -p xwire`.
2. Frontend: `vite.config.ts` (port 1420, strictPort), `tsconfig.json`, `index.html`, `src/`:
   tokens.css generated from `design/generator/lib.py` TOKENS (dark default, `[data-theme=light]`), icons.ts from
   ICON_SRC, components per `design/generator/ui.py` (sizes there), screens per `workspace.py` / `onboarding.py`:
   toolbar + custom window controls, sidebar (App/Domain/Device, pinned, favourites, saved sessions), filter bar with
   query language (status:5xx host:*.x method:POST body:"…", negation, autocomplete) + quick chips, virtualized
   table (24 px rows, column picker, sort, new-requests pill), inspector (request/response tabs, Pretty/Raw/Tree/Hex/
   Preview), context menu (Copy as cURL/fetch/Python…), toasts, status bar, keyboard shortcuts (Ctrl+L, Space,
   Ctrl+K, ?), onboarding 3 steps, 1024 layout (icon rail + bottom inspector).
3. Run with `npx tauri dev`; compare against `design/screens/*.png`.
4. Later (not designed yet): Breakpoints, Map Local/Remote, Rewrite, Scripts, Compose, Diff, Throttle, WS frames, HAR import.

Known nits: WS close reports benign "close_notify" error; tunnel/WS frame parsing not done.
