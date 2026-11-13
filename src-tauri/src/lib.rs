//! Tauri shell: owns the proxy, CA and settings, and exposes them to the UI as commands and events.
//!
//! Events emitted to the webview:
//! - `flows`  — `StoreEvent[]`, batched every ~60 ms (upserts for the same id coalesced)
//! - `resync` — the event feed lagged; the UI should call `list_flows` again
//! - `stats`  — `Stats`, once a second
//! - `proxy`  — `ProxyStatus`, whenever the proxy (re)starts or fails to

mod har;
mod settings;
mod system;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use base64::Engine;
use parking_lot::RwLock;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::broadcast::error::RecvError;
use xwire_core::store::{Headers, Timing, TlsInfo};
use xwire_core::{CaInfo, CertAuthority, FlowSummary, InterceptConfig, Proxy, ProxyOptions, Store, StoreEvent};

use settings::Settings;

const MAX_FLOWS: usize = 200_000;
/// Bodies above this are sent to the UI clipped; the full body stays in the store (and in HAR exports).
const DISPLAY_CAP: usize = 4 << 20;

pub struct App {
    ca: RwLock<Arc<CertAuthority>>,
    store: Arc<Store>,
    proxy: RwLock<Option<Arc<Proxy>>>,
    proxy_error: RwLock<Option<String>>,
    settings: RwLock<Settings>,
    /// True when xwire turned the system proxy on, so it gets switched off again on quit.
    set_system_proxy: AtomicBool,
    rules: xwire_core::rules::SharedRules,
    breaks: Arc<xwire_core::Breaks>,
}

type R<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyStatus {
    running: bool,
    addr: Option<String>,
    port: u16,
    listen_all: bool,
    error: Option<String>,
    lan_ip: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Stats {
    flows: usize,
    bytes: u64,
    requests_total: u64,
    recording: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppStateDto {
    version: &'static str,
    proxy: ProxyStatus,
    ca: CaInfo,
    settings: Settings,
    os: system::OsInfo,
    recording: bool,
}

impl App {
    fn proxy_status(&self) -> ProxyStatus {
        let s = self.settings.read();
        let p = self.proxy.read();
        ProxyStatus {
            running: p.is_some(),
            addr: p.as_ref().map(|p| p.local_addr().to_string()),
            port: s.port,
            listen_all: s.listen_all,
            error: self.proxy_error.read().clone(),
            lan_ip: system::lan_ip(),
        }
    }

    fn stats(&self) -> Stats {
        let (flows, bytes) = self.store.stats();
        let requests_total = self.proxy.read().as_ref().map(|p| p.requests_total()).unwrap_or(0);
        Stats { flows, bytes, requests_total, recording: self.store.recording() }
    }

    fn current_proxy(&self) -> R<Arc<Proxy>> {
        self.proxy.read().clone().ok_or_else(|| "The proxy is not running".to_string())
    }

    /// Stop the running proxy (if any) and start a new one from the current settings and CA.
    async fn restart(&self, app: &AppHandle) -> ProxyStatus {
        let old = self.proxy.write().take();
        let had_old = old.is_some();
        drop(old); // dropping the last Arc signals the accept loop to stop
        let (port, listen_all, intercept) = {
            let s = self.settings.read();
            (s.port, s.listen_all, s.intercept.clone())
        };
        let ip = if listen_all { "0.0.0.0" } else { "127.0.0.1" };
        let listen: SocketAddr = format!("{ip}:{port}").parse().expect("valid socket address");
        let ca = self.ca.read().clone();
        let mut last_err = None;
        // the previous listener closes asynchronously; give it a moment to release the port
        for attempt in 0..if had_old { 10 } else { 1 } {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            match Proxy::start(ca.clone(), self.store.clone(), ProxyOptions {
                    listen,
                    intercept: intercept.clone(),
                    rules: self.rules.clone(),
                    breaks: self.breaks.clone(),
                    ..Default::default()
                }).await {
                Ok(p) => {
                    *self.proxy.write() = Some(Arc::new(p));
                    last_err = None;
                    break;
                }
                Err(e) => last_err = Some(describe_bind_error(&e, port)),
            }
        }
        *self.proxy_error.write() = last_err;
        let st = self.proxy_status();
        let _ = app.emit("proxy", &st);
        st
    }
}

fn describe_bind_error(e: &anyhow::Error, port: u16) -> String {
    if let Some(io) = e.downcast_ref::<std::io::Error>() {
        if io.kind() == std::io::ErrorKind::AddrInUse {
            let owner = system::check_port(port, None).owner;
            return match owner {
                Some(o) => format!("Port {port} is already in use by {o}"),
                None => format!("Port {port} is already in use"),
            };
        }
        if io.kind() == std::io::ErrorKind::PermissionDenied {
            return format!("Not permitted to listen on port {port}");
        }
    }
    format!("Could not start the proxy on port {port}: {e}")
}

// ---------------------------------------------------------------- event forwarding

fn spawn_event_forwarder(app: AppHandle, store: Arc<Store>) {
    let mut rx = store.subscribe();
    tauri::async_runtime::spawn(async move {
        let mut buf: Vec<StoreEvent> = Vec::new();
        // id → index in buf of the latest upsert since the last Removed/Cleared
        let mut idx: HashMap<u64, usize> = HashMap::new();
        let mut tick = tokio::time::interval(Duration::from_millis(60));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                r = rx.recv() => match r {
                    Ok(ev) => match ev {
                        StoreEvent::Upsert { flow } => {
                            if let Some(&i) = idx.get(&flow.id) {
                                buf[i] = StoreEvent::Upsert { flow };
                            } else {
                                idx.insert(flow.id, buf.len());
                                buf.push(StoreEvent::Upsert { flow });
                            }
                        }
                        StoreEvent::Cleared => {
                            buf.clear();
                            idx.clear();
                            buf.push(StoreEvent::Cleared);
                        }
                        ev @ StoreEvent::Removed { .. } => {
                            idx.clear();
                            buf.push(ev);
                        }
                    },
                    Err(RecvError::Lagged(_)) => {
                        buf.clear();
                        idx.clear();
                        let _ = app.emit("resync", ());
                    }
                    Err(RecvError::Closed) => break,
                },
                _ = tick.tick() => {
                    if !buf.is_empty() {
                        idx.clear();
                        let _ = app.emit("flows", std::mem::take(&mut buf));
                    }
                }
            }
        }
    });
}

fn spawn_stats(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        loop {
            tick.tick().await;
            let st = app.state::<App>().stats();
            let _ = app.emit("stats", st);
        }
    });
}

// ---------------------------------------------------------------- flow commands

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BodyView {
    /// Size as captured (possibly compressed).
    size: usize,
    /// Size after undoing Content-Encoding.
    decoded_size: usize,
    /// UTF-8 text when the decoded body is valid UTF-8.
    text: Option<String>,
    /// Base64 of the decoded body when it is not text.
    base64: Option<String>,
    /// The proxy stopped capturing (body larger than the capture cap).
    truncated: bool,
    /// Only the first DISPLAY_CAP bytes are included here.
    clipped: bool,
    decode_error: Option<String>,
}

fn body_view(raw: &[u8], headers: &Headers, truncated: bool) -> BodyView {
    let (decoded, decode_error) = xwire_core::proxy::decoded_body(raw, headers);
    let decoded_size = decoded.len();
    let clipped = decoded.len() > DISPLAY_CAP;
    let mut slice = &decoded[..decoded.len().min(DISPLAY_CAP)];
    let (text, base64) = match std::str::from_utf8(slice) {
        Ok(s) => (Some(s.to_string()), None),
        Err(e) if clipped && e.error_len().is_none() => {
            // clipped mid-character
            slice = &slice[..e.valid_up_to()];
            (Some(String::from_utf8_lossy(slice).into_owned()), None)
        }
        Err(_) => (None, Some(base64::engine::general_purpose::STANDARD.encode(slice))),
    };
    BodyView { size: raw.len(), decoded_size, text, base64, truncated, clipped, decode_error }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FlowDetail {
    summary: FlowSummary,
    req_headers: Headers,
    req_body: BodyView,
    status_text: Option<String>,
    resp_http_version: Option<String>,
    resp_headers: Headers,
    resp_body: BodyView,
    timing: Timing,
    tls: Option<TlsInfo>,
}

#[tauri::command]
fn list_flows(app: State<App>) -> Vec<FlowSummary> {
    app.store.summaries()
}

#[tauri::command]
fn flow_detail(app: State<App>, id: u64) -> R<FlowDetail> {
    app.store
        .with(id, |f| FlowDetail {
            summary: f.summary.clone(),
            req_headers: f.req_headers.clone(),
            req_body: body_view(&f.req_body, &f.req_headers, f.req_body_truncated),
            status_text: f.status_text.clone(),
            resp_http_version: f.resp_http_version.clone(),
            resp_headers: f.resp_headers.clone(),
            resp_body: body_view(&f.resp_body, &f.resp_headers, f.resp_body_truncated),
            timing: f.timing.clone(),
            tls: f.tls.clone(),
        })
        .ok_or_else(|| format!("Flow {id} no longer exists"))
}

/// Raw (decoded) body bytes, for "Save body as…".
#[tauri::command]
fn save_body(app: State<App>, id: u64, response: bool, path: String) -> R<()> {
    let bytes = app
        .store
        .with(id, |f| {
            if response {
                xwire_core::proxy::decoded_body(&f.resp_body, &f.resp_headers).0
            } else {
                xwire_core::proxy::decoded_body(&f.req_body, &f.req_headers).0
            }
        })
        .ok_or_else(|| format!("Flow {id} no longer exists"))?;
    std::fs::write(&path, bytes).map_err(err)
}

/// Ids of flows whose decoded request or response body contains `needle` (case-insensitive).
#[tauri::command]
async fn search_bodies(app: State<'_, App>, needle: String) -> R<Vec<u64>> {
    let store = app.store.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let n = needle.to_lowercase().into_bytes();
        if n.is_empty() {
            return vec![];
        }
        let hit = |hay: &[u8]| hay.windows(n.len()).any(|w| w.eq_ignore_ascii_case(&n));
        store
            .ids()
            .into_iter()
            .filter(|id| {
                store
                    .with(*id, |f| {
                        let (rq, _) = xwire_core::proxy::decoded_body(&f.req_body, &f.req_headers);
                        if hit(&rq) {
                            return true;
                        }
                        let (rs, _) = xwire_core::proxy::decoded_body(&f.resp_body, &f.resp_headers);
                        hit(&rs)
                    })
                    .unwrap_or(false)
            })
            .collect()
    })
    .await
    .map_err(err)
}

#[tauri::command]
fn set_recording(app: State<App>, on: bool) -> bool {
    app.store.set_recording(on);
    on
}

#[tauri::command]
fn clear_flows(app: State<App>) {
    app.store.clear();
}

#[tauri::command]
fn remove_flows(app: State<App>, ids: Vec<u64>) {
    app.store.remove(&ids);
}

#[tauri::command]
fn replay(app: State<App>, id: u64) -> R<u64> {
    app.current_proxy()?.replay(id).map_err(err)
}

/// Compose / edit-and-repeat: send a new request through the proxy (rules apply).
#[tauri::command]
fn send_request(app: State<App>, method: String, url: String, headers: Headers, body: String) -> R<u64> {
    app.current_proxy()?.send(&method, &url, &headers, body.into_bytes().into(), "xwire · compose").map_err(err)
}

// ---------------------------------------------------------------- rules & breakpoints

#[tauri::command]
fn get_rules(app: State<App>) -> xwire_core::Rules {
    (**app.rules.read()).clone()
}

#[tauri::command]
fn set_rules(app: State<App>, rules: xwire_core::Rules) {
    *app.rules.write() = Arc::new(rules.clone());
    let mut s = app.settings.write();
    s.rules = rules;
    s.save();
}

#[tauri::command]
fn paused(app: State<App>) -> Vec<xwire_core::Paused> {
    app.breaks.list()
}

#[tauri::command]
fn resume(app: State<App>, id: u64, resume: xwire_core::Resume) -> bool {
    app.breaks.resume(id, resume)
}

#[tauri::command]
fn resume_all(app: State<App>) {
    app.breaks.resume_all();
}

#[tauri::command]
fn ws_messages(app: State<App>, id: u64) -> R<Vec<xwire_core::WsMessage>> {
    app.store.with(id, |f| f.ws.clone()).ok_or_else(|| format!("Flow {id} no longer exists"))
}

// ---------------------------------------------------------------- HAR import & saved sessions

#[tauri::command]
async fn import_har(app: State<'_, App>, path: String) -> R<usize> {
    let store = app.store.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let text = std::fs::read_to_string(&path).map_err(err)?;
        har::import(&store, &text, &path)
    })
    .await
    .map_err(err)?
}

fn sessions_dir() -> std::path::PathBuf {
    dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("xwire").join("sessions")
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionFile {
    name: String,
    path: String,
    size: u64,
    /// Unix epoch, milliseconds.
    modified: f64,
}

#[tauri::command]
fn list_sessions() -> Vec<SessionFile> {
    let mut out: Vec<SessionFile> = std::fs::read_dir(sessions_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().map(|x| x == "har").unwrap_or(false))
        .filter_map(|e| {
            let md = e.metadata().ok()?;
            let modified = md.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs_f64() * 1000.0;
            Some(SessionFile {
                name: e.path().file_stem()?.to_string_lossy().into_owned(),
                path: e.path().display().to_string(),
                size: md.len(),
                modified,
            })
        })
        .collect();
    out.sort_by(|a, b| b.modified.total_cmp(&a.modified));
    out
}

/// Save the current capture (or `ids`) as a named session (a HAR file in the sessions folder).
#[tauri::command]
fn save_session(app: State<App>, name: String, ids: Option<Vec<u64>>) -> R<SessionFile> {
    let clean: String = name.trim().chars().map(|c| if c.is_alphanumeric() || "-_. ".contains(c) { c } else { '-' }).collect();
    if clean.is_empty() {
        return Err("Give the session a name".into());
    }
    let dir = sessions_dir();
    std::fs::create_dir_all(&dir).map_err(err)?;
    let path = dir.join(format!("{clean}.har"));
    let ids = ids.unwrap_or_else(|| app.store.ids());
    let flows: Vec<_> = ids.iter().filter_map(|id| app.store.get(*id)).collect();
    let s = serde_json::to_string(&har::build(&flows, env!("CARGO_PKG_VERSION"))).map_err(err)?;
    std::fs::write(&path, &s).map_err(err)?;
    Ok(SessionFile { name: clean, path: path.display().to_string(), size: s.len() as u64, modified: now_ms() })
}

#[tauri::command]
fn delete_session(path: String) -> R<()> {
    let p = std::path::PathBuf::from(&path);
    // only ever delete inside the sessions folder
    if p.parent() != Some(sessions_dir().as_path()) {
        return Err("Not a saved session".into());
    }
    std::fs::remove_file(p).map_err(err)
}

fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0)
}

#[tauri::command]
fn export_har(app: State<App>, ids: Option<Vec<u64>>, path: String) -> R<usize> {
    let ids = ids.unwrap_or_else(|| app.store.ids());
    let flows: Vec<_> = ids.iter().filter_map(|id| app.store.get(*id)).collect();
    let har = har::build(&flows, env!("CARGO_PKG_VERSION"));
    let s = serde_json::to_string_pretty(&har).map_err(err)?;
    std::fs::write(&path, s).map_err(err)?;
    Ok(flows.len())
}

// ---------------------------------------------------------------- app / proxy commands

#[tauri::command]
fn app_state(app: State<App>) -> AppStateDto {
    AppStateDto {
        version: env!("CARGO_PKG_VERSION"),
        proxy: app.proxy_status(),
        ca: app.ca.read().info().clone(),
        settings: app.settings.read().clone(),
        os: system::os_info(),
        recording: app.store.recording(),
    }
}

#[tauri::command]
fn stats(app: State<App>) -> Stats {
    app.stats()
}

#[tauri::command]
async fn restart_proxy(handle: AppHandle, app: State<'_, App>, port: u16, listen_all: bool) -> R<ProxyStatus> {
    {
        let mut s = app.settings.write();
        s.port = port;
        s.listen_all = listen_all;
        s.save();
    }
    Ok(app.restart(&handle).await)
}

#[tauri::command]
fn set_intercept(app: State<App>, cfg: InterceptConfig) {
    if let Some(p) = app.proxy.read().as_ref() {
        p.set_intercept(cfg.clone());
    }
    let mut s = app.settings.write();
    s.intercept = cfg;
    s.save();
}

/// Persist UI preferences. Only the fields present in `patch` change.
#[tauri::command]
fn set_prefs(app: State<App>, patch: serde_json::Value) -> R<Settings> {
    let mut s = app.settings.write();
    let mut v = serde_json::to_value(&*s).map_err(err)?;
    if let (Some(obj), Some(p)) = (v.as_object_mut(), patch.as_object()) {
        for (k, val) in p {
            // proxy-affecting fields go through their own commands
            if matches!(k.as_str(), "port" | "listenAll" | "intercept" | "rules") {
                continue;
            }
            obj.insert(k.clone(), val.clone());
        }
    }
    *s = serde_json::from_value(v).map_err(err)?;
    s.save();
    Ok(s.clone())
}

#[tauri::command]
fn check_port(app: State<App>, port: u16) -> system::PortCheck {
    let ours = app.proxy.read().as_ref().map(|p| p.local_addr().port());
    system::check_port(port, ours)
}

#[tauri::command]
fn lan_ip() -> Option<String> {
    system::lan_ip()
}

// ---------------------------------------------------------------- certificate commands

#[tauri::command]
fn ca_info(app: State<App>) -> CaInfo {
    app.ca.read().info().clone()
}

#[tauri::command]
fn ca_pem(app: State<App>) -> String {
    app.ca.read().pem().to_string()
}

#[tauri::command]
async fn trust_status(app: State<'_, App>) -> R<Vec<system::TrustEntry>> {
    let info = app.ca.read().info().clone();
    tauri::async_runtime::spawn_blocking(move || system::trust_status(&info.cert_path, &info.common_name))
        .await
        .map_err(err)
}

#[tauri::command]
async fn install_ca_system(app: State<'_, App>) -> R<String> {
    let dir = app.ca.read().info().dir.clone();
    tauri::async_runtime::spawn_blocking(move || system::install_system(&dir)).await.map_err(err)?
}

#[tauri::command]
async fn install_nss(app: State<'_, App>, firefox: bool) -> R<String> {
    let dir = app.ca.read().info().dir.clone();
    tauri::async_runtime::spawn_blocking(move || system::install_nss(&dir, firefox)).await.map_err(err)?
}

#[tauri::command]
fn manual_commands(app: State<App>) -> serde_json::Value {
    system::manual_commands(&app.ca.read().info().dir)
}

/// Write the CA certificate as `pem` (also used for .crt) or `der`.
#[tauri::command]
fn export_ca(app: State<App>, format: String, path: String) -> R<()> {
    let ca = app.ca.read().clone();
    match format.as_str() {
        "der" => std::fs::write(&path, ca.der()),
        _ => std::fs::write(&path, ca.pem()),
    }
    .map_err(err)
}

/// Fetch https://xwire.check through the proxy with curl, which uses the system trust store.
#[tauri::command]
async fn verify_interception(app: State<'_, App>) -> R<String> {
    let addr = app.current_proxy()?.local_addr();
    let proxy = format!("http://127.0.0.1:{}", addr.port());
    tauri::async_runtime::spawn_blocking(move || {
        let out = std::process::Command::new("curl")
            .args(["-sS", "--max-time", "8", "-o", "/dev/null", "-w", "%{http_code}", "-x", &proxy, "https://xwire.check"])
            .output()
            .map_err(|e| format!("curl not available: {e}"))?;
        let code = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if out.status.success() && code == "200" {
            Ok("curl https://xwire.check returned 200 through the proxy".to_string())
        } else {
            let e = String::from_utf8_lossy(&out.stderr).trim().to_string();
            Err(if e.is_empty() { format!("xwire.check returned {code}") } else { e })
        }
    })
    .await
    .map_err(err)?
}

/// QR code for `text` as an SVG document.
#[tauri::command]
fn qr_svg(text: String) -> R<String> {
    let code = qrcode::QrCode::new(text.as_bytes()).map_err(err)?;
    Ok(code
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(180, 180)
        .quiet_zone(true)
        .dark_color(qrcode::render::svg::Color("#0a0c0f"))
        .light_color(qrcode::render::svg::Color("#ffffff"))
        .build())
}

/// Replace the root CA with a fresh one. Devices that trusted the old one must trust the new one again.
#[tauri::command]
async fn regenerate_ca(handle: AppHandle, app: State<'_, App>) -> R<CaInfo> {
    let dir = std::path::PathBuf::from(&app.ca.read().info().dir);
    let ca = tauri::async_runtime::spawn_blocking(move || CertAuthority::generate(&dir)).await.map_err(err)?.map_err(err)?;
    let info = ca.info().clone();
    *app.ca.write() = Arc::new(ca);
    app.restart(&handle).await;
    Ok(info)
}

// ---------------------------------------------------------------- system proxy

#[tauri::command]
async fn system_proxy(app: State<'_, App>) -> R<system::SystemProxy> {
    let port = app.settings.read().port;
    tauri::async_runtime::spawn_blocking(move || system::system_proxy_status(port)).await.map_err(err)
}

#[tauri::command]
async fn set_system_proxy(app: State<'_, App>, on: bool) -> R<system::SystemProxy> {
    let (port, bypass) = {
        let s = app.settings.read();
        (s.port, s.system_proxy_bypass.clone())
    };
    let r = tauri::async_runtime::spawn_blocking(move || system::set_system_proxy(on, port, &bypass)).await.map_err(err)??;
    app.set_system_proxy.store(on, Ordering::Relaxed);
    Ok(r)
}

// ---------------------------------------------------------------- entry point

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info,xwire_core=info".into()))
        .init();

    let settings = Settings::load();
    let ca_dir = std::env::var("XWIRE_CA_DIR").map(Into::into).unwrap_or_else(|_| xwire_core::ca::default_ca_dir());
    let ca = match CertAuthority::load_or_create(&ca_dir) {
        Ok(ca) => ca,
        Err(e) => {
            tracing::error!("CA at {} unusable ({e:#}); generating a new one", ca_dir.display());
            CertAuthority::generate(&ca_dir).expect("cannot create a root CA")
        }
    };
    let store = Store::new(MAX_FLOWS);
    let rules = settings.rules.clone();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .manage(App {
            ca: RwLock::new(Arc::new(ca)),
            store: store.clone(),
            proxy: RwLock::new(None),
            proxy_error: RwLock::new(None),
            settings: RwLock::new(settings),
            set_system_proxy: AtomicBool::new(false),
            rules: Arc::new(RwLock::new(Arc::new(rules))),
            breaks: Arc::new(xwire_core::Breaks::default()),
        })
        .setup(move |app| {
            let handle = app.handle().clone();
            spawn_event_forwarder(handle.clone(), store.clone());
            spawn_stats(handle.clone());
            tauri::async_runtime::spawn(async move {
                let st = handle.state::<App>().restart(&handle).await;
                match &st.error {
                    Some(e) => tracing::error!("{e}"),
                    None => tracing::info!("proxy on {}", st.addr.unwrap_or_default()),
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_state,
            stats,
            list_flows,
            flow_detail,
            search_bodies,
            export_ca,
            verify_interception,
            qr_svg,
            save_body,
            set_recording,
            clear_flows,
            remove_flows,
            replay,
            send_request,
            get_rules,
            set_rules,
            paused,
            resume,
            resume_all,
            ws_messages,
            import_har,
            list_sessions,
            save_session,
            delete_session,
            export_har,
            restart_proxy,
            set_intercept,
            set_prefs,
            check_port,
            lan_ip,
            ca_info,
            ca_pem,
            trust_status,
            install_ca_system,
            install_nss,
            manual_commands,
            regenerate_ca,
            system_proxy,
            set_system_proxy,
        ])
        .build(tauri::generate_context!())
        .expect("error while building xwire");

    app.run(|handle, event| {
        if let tauri::RunEvent::Exit = event {
            let st = handle.state::<App>();
            if st.set_system_proxy.load(Ordering::Relaxed) {
                let (port, bypass) = {
                    let s = st.settings.read();
                    (s.port, s.system_proxy_bypass.clone())
                };
                if let Err(e) = system::set_system_proxy(false, port, &bypass) {
                    tracing::warn!("restoring system proxy: {e}");
                }
            }
        }
    });
}
