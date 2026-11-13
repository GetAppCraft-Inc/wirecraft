//! The HTTP(S) proxy: plain HTTP forwarding, CONNECT tunnels (decrypted with a minted certificate or passed
//! through blind), HTTP/1.1 and HTTP/2 towards both client and upstream, and WebSocket upgrades.

use std::convert::Infallible;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use bytes::Bytes;
use http::header::{self, HeaderMap, HeaderName, HeaderValue};
use http::{Method, Request, Response, StatusCode, Uri, Version};
use http_body_util::combinators::UnsyncBoxBody;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::{TokioExecutor, TokioIo};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;

use crate::body::{TeeBody, TeeDone, classify, decode};
use crate::ca::CertAuthority;
use crate::procinfo;
use crate::store::{Flow, FlowState, FlowSummary, Headers, Store, Timing, TlsInfo};

type BoxError = Box<dyn std::error::Error + Send + Sync>;
pub type RespBody = UnsyncBoxBody<Bytes, BoxError>;
type Upstream = Client<hyper_rustls::HttpsConnector<HttpConnector>, Full<Bytes>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InterceptMode {
    /// Decrypt every host except the never-decrypt list.
    All,
    /// Decrypt only hosts in the listed set.
    Listed,
    /// Tunnel all HTTPS untouched.
    Off,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterceptConfig {
    pub mode: InterceptMode,
    pub listed: Vec<String>,
    pub never: Vec<String>,
}

impl Default for InterceptConfig {
    fn default() -> Self {
        Self { mode: InterceptMode::All, listed: vec![], never: vec![] }
    }
}

impl InterceptConfig {
    pub fn should_decrypt(&self, host: &str) -> bool {
        if self.never.iter().any(|p| glob_match(p, host)) {
            return false;
        }
        match self.mode {
            InterceptMode::All => true,
            InterceptMode::Listed => self.listed.iter().any(|p| glob_match(p, host)),
            InterceptMode::Off => false,
        }
    }
}

/// `*` matches any run of characters; matching is case-insensitive. `*.example.com` also matches `example.com`.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let p = pattern.trim().to_ascii_lowercase();
    let t = text.to_ascii_lowercase();
    if let Some(rest) = p.strip_prefix("*.") {
        if t == rest {
            return true;
        }
    }
    fn m(p: &[u8], t: &[u8]) -> bool {
        match p.first() {
            None => t.is_empty(),
            Some(b'*') => (0..=t.len()).any(|i| m(&p[1..], &t[i..])),
            Some(c) => t.first() == Some(c) && m(&p[1..], &t[1..]),
        }
    }
    m(p.as_bytes(), t.as_bytes())
}

#[derive(Debug, Clone)]
pub struct ProxyOptions {
    pub listen: SocketAddr,
    pub intercept: InterceptConfig,
    /// Bodies larger than this are forwarded in full but only this much is kept for inspection.
    pub body_cap: usize,
}

impl Default for ProxyOptions {
    fn default() -> Self {
        Self { listen: "127.0.0.1:9090".parse().unwrap(), intercept: InterceptConfig::default(), body_cap: 16 << 20 }
    }
}

struct Shared {
    ca: Arc<CertAuthority>,
    store: Arc<Store>,
    client: Upstream,
    /// HTTP/1.1-only client: upgrades (WebSocket) can't go over h2.
    client_h1: Upstream,
    intercept: RwLock<InterceptConfig>,
    body_cap: usize,
    local: SocketAddr,
    requests: AtomicU64,
}

pub struct Proxy {
    shared: Arc<Shared>,
    stop: Option<oneshot::Sender<()>>,
}

#[derive(Clone)]
struct Conn {
    peer: SocketAddr,
    /// Owning process, filled in by a background lookup when the connection opens.
    process: Arc<OnceLock<Option<String>>>,
    scheme: &'static str,
    /// Target of the CONNECT this connection was decrypted from.
    target: Option<(String, u16)>,
    tls: Option<TlsInfo>,
}

impl Conn {
    fn client_name(&self, ua: Option<&str>) -> Option<String> {
        if let Some(Some(p)) = self.process.get() {
            return Some(p.clone());
        }
        ua.and_then(procinfo::from_user_agent)
    }
}

fn now_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0)
}

fn upstream_client(h2: bool) -> Result<Upstream> {
    let mut roots = rustls::RootCertStore::empty();
    let native = rustls_native_certs::load_native_certs();
    for c in native.certs {
        let _ = roots.add(c);
    }
    if roots.is_empty() {
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    }
    let tls = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()?
        .with_root_certificates(roots)
        .with_no_client_auth();
    let mut http = HttpConnector::new();
    http.enforce_http(false);
    http.set_nodelay(true);
    http.set_connect_timeout(Some(Duration::from_secs(15)));
    let b = hyper_rustls::HttpsConnectorBuilder::new().with_tls_config(tls).https_or_http().enable_http1();
    let https = if h2 { b.enable_http2().wrap_connector(http) } else { b.wrap_connector(http) };
    Ok(Client::builder(TokioExecutor::new()).pool_idle_timeout(Duration::from_secs(60)).build(https))
}

impl Proxy {
    pub async fn start(ca: Arc<CertAuthority>, store: Arc<Store>, opts: ProxyOptions) -> Result<Self> {
        let listener = TcpListener::bind(opts.listen).await?;
        let local = listener.local_addr()?;
        let shared = Arc::new(Shared {
            ca,
            store,
            client: upstream_client(true)?,
            client_h1: upstream_client(false)?,
            intercept: RwLock::new(opts.intercept),
            body_cap: opts.body_cap,
            local,
            requests: AtomicU64::new(0),
        });
        let (stop, mut stopped) = oneshot::channel();
        let sh = shared.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut stopped => break,
                    r = listener.accept() => match r {
                        Ok((stream, peer)) => {
                            let sh = sh.clone();
                            tokio::spawn(async move { serve_plain(sh, stream, peer).await });
                        }
                        Err(e) => {
                            tracing::warn!("accept: {e}");
                            tokio::time::sleep(Duration::from_millis(50)).await;
                        }
                    }
                }
            }
        });
        tracing::info!("xwire proxy listening on {local}");
        Ok(Self { shared, stop: Some(stop) })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.shared.local
    }

    pub fn store(&self) -> &Arc<Store> {
        &self.shared.store
    }

    pub fn ca(&self) -> &Arc<CertAuthority> {
        &self.shared.ca
    }

    pub fn requests_total(&self) -> u64 {
        self.shared.requests.load(Ordering::Relaxed)
    }

    pub fn intercept(&self) -> InterceptConfig {
        self.shared.intercept.read().clone()
    }

    pub fn set_intercept(&self, cfg: InterceptConfig) {
        *self.shared.intercept.write() = cfg;
    }

    pub fn stop(mut self) {
        if let Some(s) = self.stop.take() {
            let _ = s.send(());
        }
    }

    /// Send a captured request again. The new exchange is recorded as its own flow; returns its id.
    pub async fn replay(&self, id: u64) -> Result<u64> {
        let f = self.shared.store.get(id).ok_or_else(|| anyhow::anyhow!("flow {id} no longer exists"))?;
        let mut headers = HeaderMap::new();
        for (k, v) in &f.req_headers {
            if k.starts_with(':') {
                continue;
            }
            if let (Ok(k), Ok(v)) = (HeaderName::from_bytes(k.as_bytes()), HeaderValue::from_str(v)) {
                headers.append(k, v);
            }
        }
        let s = &f.summary;
        let out = Outgoing {
            method: Method::from_bytes(s.method.as_bytes())?,
            scheme: if s.scheme == "https" { "https" } else { "http" },
            host: s.host.clone(),
            port: s.port,
            path: s.path.clone(),
            version: s.http_version.clone(),
            headers,
            body: f.req_body.clone(),
            client: Some("xwire · repeat".into()),
            client_addr: "xwire".into(),
            tls: f.tls.clone(),
            started: Instant::now(),
            started_unix: now_ms(),
            request_ms: 0.0,
        };
        let new_id = self.shared.store.next_id();
        let resp = exchange(self.shared.clone(), out, None, Some(new_id)).await;
        // drain so the tee records the body and closes the flow
        let _ = resp.into_body().collect().await;
        Ok(new_id)
    }
}

impl Drop for Proxy {
    fn drop(&mut self) {
        if let Some(s) = self.stop.take() {
            let _ = s.send(());
        }
    }
}

async fn serve_plain(sh: Arc<Shared>, stream: TcpStream, peer: SocketAddr) {
    let _ = stream.set_nodelay(true);
    let process = Arc::new(OnceLock::new());
    {
        let process = process.clone();
        let local = sh.local;
        tokio::task::spawn_blocking(move || {
            let name = procinfo::lookup(peer, local).map(|p| procinfo::pretty(&p.name));
            let _ = process.set(name);
        });
    }
    let conn = Conn { peer, process, scheme: "http", target: None, tls: None };
    let svc = service_fn(move |req| {
        let sh = sh.clone();
        let conn = conn.clone();
        async move { Ok::<_, Infallible>(handle(sh, conn, req).await) }
    });
    let r = hyper::server::conn::http1::Builder::new()
        .preserve_header_case(true)
        .serve_connection(TokioIo::new(stream), svc)
        .with_upgrades()
        .await;
    if let Err(e) = r {
        tracing::debug!("client connection {peer}: {e}");
    }
}

async fn handle(sh: Arc<Shared>, conn: Conn, req: Request<Incoming>) -> Response<RespBody> {
    if req.method() == Method::CONNECT {
        return handle_connect(sh, conn, req);
    }
    proxy_request(sh, conn, req).await
}

fn full(b: impl Into<Bytes>) -> RespBody {
    Full::new(b.into()).map_err(|never: Infallible| match never {}).boxed_unsync()
}

fn text_response(status: StatusCode, msg: impl Into<String>) -> Response<RespBody> {
    let mut r = Response::new(full(msg.into()));
    *r.status_mut() = status;
    r.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("text/plain; charset=utf-8"));
    r
}

fn split_host_port(authority: &str, default_port: u16) -> (String, u16) {
    if let Some(rest) = authority.strip_prefix('[') {
        // [v6]:port
        if let Some((h, p)) = rest.split_once(']') {
            let port = p.strip_prefix(':').and_then(|p| p.parse().ok()).unwrap_or(default_port);
            return (h.to_string(), port);
        }
    }
    match authority.rsplit_once(':') {
        Some((h, p)) if !h.contains(':') => (h.to_string(), p.parse().unwrap_or(default_port)),
        _ => (authority.to_string(), default_port),
    }
}

fn handle_connect(sh: Arc<Shared>, conn: Conn, mut req: Request<Incoming>) -> Response<RespBody> {
    let authority = req.uri().authority().map(|a| a.to_string()).unwrap_or_default();
    let (host, port) = split_host_port(&authority, 443);
    if host.is_empty() {
        return text_response(StatusCode::BAD_REQUEST, "CONNECT needs host:port");
    }
    let decrypt = sh.intercept.read().should_decrypt(&host);
    let ua = req.headers().get(header::USER_AGENT).and_then(|v| v.to_str().ok()).map(|s| s.to_string());
    let on_upgrade = hyper::upgrade::on(&mut req);
    tokio::spawn(async move {
        let upgraded = match on_upgrade.await {
            Ok(u) => u,
            Err(e) => {
                tracing::debug!("CONNECT upgrade {host}: {e}");
                return;
            }
        };
        let io = TokioIo::new(upgraded);
        if decrypt {
            intercept_tunnel(sh, conn, io, host, port, ua).await;
        } else {
            blind_tunnel(sh, conn, io, host, port, ua, None).await;
        }
    });
    Response::new(full(Bytes::new()))
}

fn tunnel_flow(sh: &Shared, conn: &Conn, host: &str, port: u16, ua: Option<&str>) -> Option<u64> {
    if !sh.store.recording() {
        return None;
    }
    let id = sh.store.next_id();
    let started = now_ms();
    sh.store.insert(Flow {
        summary: FlowSummary {
            id,
            method: "CONNECT".into(),
            scheme: "https".into(),
            host: host.to_string(),
            port,
            path: format!(":{port}"),
            http_version: "HTTP/1.1".into(),
            status: Some(200),
            state: FlowState::Tunnel,
            kind: "TLS".into(),
            content_type: None,
            req_size: 0,
            resp_size: 0,
            started,
            duration_ms: None,
            client: conn.client_name(ua),
            client_addr: conn.peer.to_string(),
            tls: true,
            xhr: false,
            error: None,
            note: Some("tunnelled".into()),
            req_header_count: 0,
            resp_header_count: 0,
        },
        req_headers: vec![("host".into(), format!("{host}:{port}"))],
        req_body: Bytes::new(),
        req_body_truncated: false,
        status_text: Some("Connection Established".into()),
        resp_http_version: None,
        resp_headers: vec![],
        resp_body: Bytes::new(),
        resp_body_truncated: false,
        timing: Timing { started, ..Default::default() },
        tls: None,
    });
    Some(id)
}

/// Pass the tunnel through untouched, recording only byte counts.
async fn blind_tunnel<S>(sh: Arc<Shared>, conn: Conn, mut io: S, host: String, port: u16, ua: Option<String>, note: Option<String>)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let start = Instant::now();
    let id = tunnel_flow(&sh, &conn, &host, port, ua.as_deref());
    if let (Some(id), Some(n)) = (id, note) {
        sh.store.update(id, |f| f.summary.note = Some(n));
    }
    let result = match TcpStream::connect((host.as_str(), port)).await {
        Ok(mut up) => tokio::io::copy_bidirectional(&mut io, &mut up).await.map_err(|e| e.to_string()),
        Err(e) => Err(format!("connect {host}:{port}: {e}")),
    };
    if let Some(id) = id {
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        sh.store.update(id, |f| {
            f.summary.duration_ms = Some(ms);
            f.timing.total_ms = Some(ms);
            match &result {
                Ok((sent, recv)) => {
                    f.summary.req_size = *sent;
                    f.summary.resp_size = *recv;
                    f.summary.state = FlowState::Complete;
                }
                Err(e) => {
                    f.summary.state = FlowState::Error;
                    f.summary.error = Some(e.clone());
                }
            }
        });
    }
}

async fn intercept_tunnel<S>(sh: Arc<Shared>, conn: Conn, mut io: S, host: String, port: u16, ua: Option<String>)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    // Peek at the first byte: 0x16 is a TLS handshake record; anything else is plain HTTP through the tunnel.
    let mut first = vec![0u8; 1];
    match io.read(&mut first).await {
        Ok(0) | Err(_) => return,
        Ok(_) => {}
    }
    let io = Rewind::new(Bytes::from(first.clone()), io);
    if first[0] != 0x16 {
        if !looks_like_http(first[0]) {
            // some other protocol over CONNECT: pass it through
            return blind_tunnel(sh, conn, io, host, port, ua, Some("not HTTP".into())).await;
        }
        let conn = Conn { scheme: "http", target: Some((host, port)), ..conn };
        serve_decrypted(sh, conn, io, ua).await;
        return;
    }
    let cfg = sh.ca.server_config(&host, &[b"h2", b"http/1.1"]);
    let acceptor = tokio_rustls::TlsAcceptor::from(cfg);
    match acceptor.accept(io).await {
        Ok(tls) => {
            let (_, s) = tls.get_ref();
            let info = TlsInfo {
                version: s.protocol_version().map(|v| match v {
                    rustls::ProtocolVersion::TLSv1_3 => "TLS 1.3".to_string(),
                    rustls::ProtocolVersion::TLSv1_2 => "TLS 1.2".to_string(),
                    other => format!("{other:?}"),
                }),
                alpn: s.alpn_protocol().map(|a| String::from_utf8_lossy(a).into_owned()),
                sni: s.server_name().map(|s| s.to_string()),
                cipher: s.negotiated_cipher_suite().map(|c| format!("{:?}", c.suite())),
            };
            let conn = Conn { scheme: "https", target: Some((host, port)), tls: Some(info), ..conn };
            serve_decrypted(sh, conn, tls, ua).await;
        }
        Err(e) => record_handshake_failure(&sh, &conn, &host, port, ua.as_deref(), &e.to_string()),
    }
}

fn looks_like_http(b: u8) -> bool {
    // first byte of an HTTP/1 method, or of the HTTP/2 preface ("PRI")
    b.is_ascii_uppercase()
}

async fn serve_decrypted<S>(sh: Arc<Shared>, conn: Conn, io: S, ua: Option<String>)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let peer = conn.peer;
    let opened = Instant::now();
    let served = Arc::new(AtomicU64::new(0));
    let (sh2, conn2, served2) = (sh.clone(), conn.clone(), served.clone());
    let svc = service_fn(move |req| {
        let sh = sh2.clone();
        let conn = conn2.clone();
        served2.fetch_add(1, Ordering::Relaxed);
        async move { Ok::<_, Infallible>(proxy_request(sh, conn, req).await) }
    });
    // the auto builder detects the HTTP/2 preface itself
    let mut b = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new());
    b.http1().preserve_header_case(true);
    if let Err(e) = b.serve_connection_with_upgrades(TokioIo::new(io), svc).await {
        tracing::debug!("decrypted connection {peer}: {e}");
        // With TLS 1.3 the client rejects our certificate *after* the server side of the handshake finished,
        // so the failure surfaces here as an alert before any request.
        // An abrupt close right after the handshake with no request is how rejection looks from here; idle
        // pre-connects that time out later are not.
        let msg = format!("{e:#}");
        let rejected = opened.elapsed() < Duration::from_secs(3);
        if served.load(Ordering::Relaxed) == 0 && rejected && conn.tls.is_some() {
            let (host, port) = conn.target.clone().unwrap_or_default();
            record_handshake_failure(&sh, &conn, &host, port, ua.as_deref(), &msg);
        }
    }
}

fn record_handshake_failure(sh: &Shared, conn: &Conn, host: &str, port: u16, ua: Option<&str>, err: &str) {
    if let Some(id) = tunnel_flow(sh, conn, host, port, ua) {
        let msg = format!(
            "TLS handshake failed: {err}. The client doesn't trust the xwire root CA, or it pins certificates \
             (add the host to Never decrypt to tunnel it untouched)."
        );
        sh.store.update(id, |f| {
            f.summary.state = FlowState::Error;
            f.summary.status = None;
            f.summary.error = Some(msg);
            f.summary.note = Some("SSL handshake failed".into());
        });
    }
}

const HOP_BY_HOP: &[&str] = &[
    "connection",
    "proxy-connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];

fn strip_hop_by_hop(h: &mut HeaderMap, keep_upgrade: bool) {
    let listed: Vec<String> = h
        .get_all(header::CONNECTION)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(',').map(|s| s.trim().to_ascii_lowercase()).collect::<Vec<_>>())
        .collect();
    for name in HOP_BY_HOP.iter().copied().map(String::from).chain(listed) {
        if keep_upgrade && (name == "connection" || name == "upgrade") {
            continue;
        }
        h.remove(name.as_str());
    }
}

fn header_list(h: &HeaderMap) -> Headers {
    h.iter().map(|(k, v)| (k.as_str().to_string(), String::from_utf8_lossy(v.as_bytes()).into_owned())).collect()
}

fn version_str(v: Version) -> &'static str {
    match v {
        Version::HTTP_09 => "HTTP/0.9",
        Version::HTTP_10 => "HTTP/1.0",
        Version::HTTP_11 => "HTTP/1.1",
        Version::HTTP_2 => "h2",
        Version::HTTP_3 => "h3",
        _ => "HTTP/?",
    }
}

struct Outgoing {
    method: Method,
    scheme: &'static str,
    host: String,
    port: u16,
    path: String,
    version: String,
    headers: HeaderMap,
    body: Bytes,
    client: Option<String>,
    client_addr: String,
    tls: Option<TlsInfo>,
    started: Instant,
    started_unix: f64,
    request_ms: f64,
}

async fn proxy_request(sh: Arc<Shared>, conn: Conn, mut req: Request<Incoming>) -> Response<RespBody> {
    let started = Instant::now();
    let started_unix = now_ms();
    let version = req.version();

    // Work out where this request is going.
    let (scheme, host, port) = if let Some((thost, tport)) = &conn.target {
        let auth = req
            .uri()
            .authority()
            .map(|a| a.to_string())
            .or_else(|| req.headers().get(header::HOST).and_then(|v| v.to_str().ok()).map(|s| s.to_string()));
        let default = if conn.scheme == "https" { 443 } else { 80 };
        match auth {
            Some(a) => {
                let (h, p) = split_host_port(&a, default);
                // keep the CONNECT port when the Host header omits it
                (conn.scheme, h, if a.contains(':') && !a.ends_with(']') { p } else { *tport })
            }
            None => (conn.scheme, thost.clone(), *tport),
        }
    } else if let (Some(s), Some(a)) = (req.uri().scheme_str(), req.uri().authority()) {
        let https = s.eq_ignore_ascii_case("https");
        let (h, p) = split_host_port(a.as_str(), if https { 443 } else { 80 });
        (if https { "https" } else { "http" }, h, p)
    } else {
        // Origin-form request straight to the proxy port: serve the local landing page / certificate.
        return local_page(&sh, &req);
    };

    if let Some(r) = special_host(&sh, &host) {
        return r;
    }

    let path = req.uri().path_and_query().map(|p| p.as_str().to_string()).unwrap_or_else(|| "/".into());
    let is_upgrade = version == Version::HTTP_11 && req.headers().contains_key(header::UPGRADE);
    let on_upgrade = if is_upgrade { Some(hyper::upgrade::on(&mut req)) } else { None };
    let (parts, body) = req.into_parts();
    let body = match body.collect().await {
        Ok(b) => b.to_bytes(),
        Err(e) => return text_response(StatusCode::BAD_REQUEST, format!("xwire: reading request body: {e}")),
    };
    let request_ms = started.elapsed().as_secs_f64() * 1000.0;
    let ua = parts.headers.get(header::USER_AGENT).and_then(|v| v.to_str().ok());
    let out = Outgoing {
        method: parts.method.clone(),
        scheme,
        host,
        port,
        path,
        version: version_str(version).to_string(),
        client: conn.client_name(ua),
        headers: parts.headers,
        body,
        client_addr: conn.peer.to_string(),
        tls: conn.tls.clone(),
        started,
        started_unix,
        request_ms,
    };
    let id = sh.store.recording().then(|| sh.store.next_id());
    let process = conn.process.clone();
    let mut resp = exchange(sh.clone(), out, on_upgrade, id).await;
    // the process lookup usually finishes after the first request on a connection was recorded
    if let (Some(id), Some(Some(name))) = (id, process.get()) {
        let name = name.clone();
        sh.store.update(id, |f| f.summary.client = Some(name));
    }
    if version == Version::HTTP_2 {
        strip_hop_by_hop(resp.headers_mut(), false);
    }
    resp
}

fn display_url(o: &Outgoing) -> String {
    let default = if o.scheme == "https" { 443 } else { 80 };
    let host = if o.host.contains(':') { format!("[{}]", o.host) } else { o.host.clone() };
    if o.port == default {
        format!("{}://{}{}", o.scheme, host, o.path)
    } else {
        format!("{}://{}:{}{}", o.scheme, host, o.port, o.path)
    }
}

/// Record the request, forward it upstream, and return the response for the client (body tee'd into the store).
async fn exchange(sh: Arc<Shared>, o: Outgoing, on_upgrade: Option<hyper::upgrade::OnUpgrade>, id: Option<u64>) -> Response<RespBody> {
    sh.requests.fetch_add(1, Ordering::Relaxed);
    let url = display_url(&o);
    let req_ct = o.headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).map(|s| s.to_string());
    let is_upgrade = on_upgrade.is_some();
    let xhr = o.headers.get("sec-fetch-dest").map(|v| v.as_bytes() == b"empty").unwrap_or(false)
        || o.headers.contains_key("x-requested-with");

    if let Some(id) = id {
        let mut req_headers = Vec::new();
        if o.version == "h2" {
            req_headers.push((":method".into(), o.method.to_string()));
            req_headers.push((":scheme".into(), o.scheme.to_string()));
            req_headers.push((":authority".into(), o.host.clone()));
            req_headers.push((":path".into(), o.path.clone()));
        }
        req_headers.extend(header_list(&o.headers));
        let cap = sh.body_cap;
        let truncated = o.body.len() > cap;
        sh.store.insert(Flow {
            summary: FlowSummary {
                id,
                method: o.method.to_string(),
                scheme: o.scheme.into(),
                host: o.host.clone(),
                port: o.port,
                path: o.path.clone(),
                http_version: o.version.clone(),
                status: None,
                state: FlowState::Pending,
                kind: classify(o.method.as_str(), &o.path, req_ct.as_deref(), None, is_upgrade).into(),
                content_type: None,
                req_size: o.body.len() as u64,
                resp_size: 0,
                started: o.started_unix,
                duration_ms: None,
                client: o.client.clone(),
                client_addr: o.client_addr.clone(),
                tls: o.scheme == "https",
                xhr,
                error: None,
                note: None,
                req_header_count: req_headers.len(),
                resp_header_count: 0,
            },
            req_headers,
            req_body: if truncated { o.body.slice(..cap) } else { o.body.clone() },
            req_body_truncated: truncated,
            status_text: None,
            resp_http_version: None,
            resp_headers: vec![],
            resp_body: Bytes::new(),
            resp_body_truncated: false,
            timing: Timing { started: o.started_unix, request_ms: Some(o.request_ms), ..Default::default() },
            tls: o.tls.clone(),
        });
    }

    // Build the upstream request.
    let uri: Uri = match url.parse() {
        Ok(u) => u,
        Err(e) => return fail(&sh, id, &o, StatusCode::BAD_REQUEST, format!("invalid URL {url}: {e}")),
    };
    let mut headers = o.headers.clone();
    strip_hop_by_hop(&mut headers, is_upgrade);
    // HTTP/2 clients may split cookies across several headers; HTTP/1 servers expect one.
    let cookies: Vec<String> =
        headers.get_all(header::COOKIE).iter().filter_map(|v| v.to_str().ok()).map(|s| s.to_string()).collect();
    if cookies.len() > 1 {
        headers.remove(header::COOKIE);
        if let Ok(v) = HeaderValue::from_str(&cookies.join("; ")) {
            headers.insert(header::COOKIE, v);
        }
    }
    // The client derives Host (HTTP/1.1) or :authority (HTTP/2) from the URI; a literal Host header over h2
    // is rejected by strict servers.
    headers.remove(header::HOST);
    let mut out = Request::new(Full::new(o.body.clone()));
    *out.method_mut() = o.method.clone();
    *out.uri_mut() = uri;
    *out.version_mut() = Version::HTTP_11;
    *out.headers_mut() = headers;

    let sent_at = Instant::now();
    let mut resp = match tokio::time::timeout(Duration::from_secs(300), if is_upgrade { &sh.client_h1 } else { &sh.client }.request(out)).await {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            let mut msg = e.to_string();
            let mut src = std::error::Error::source(&e);
            while let Some(s) = src {
                msg.push_str(": ");
                msg.push_str(&s.to_string());
                src = s.source();
            }
            return fail(&sh, id, &o, StatusCode::BAD_GATEWAY, msg);
        }
        Err(_) => return fail(&sh, id, &o, StatusCode::GATEWAY_TIMEOUT, "upstream did not respond within 300 s".into()),
    };
    let waiting_ms = sent_at.elapsed().as_secs_f64() * 1000.0;
    let status = resp.status();
    let server_up = (status == StatusCode::SWITCHING_PROTOCOLS && on_upgrade.is_some()).then(|| hyper::upgrade::on(&mut resp));
    let (mut parts, body) = resp.into_parts();
    let resp_ct = parts.headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).map(|s| s.to_string());

    if let Some(id) = id {
        let mut resp_headers = Vec::new();
        if parts.version == Version::HTTP_2 {
            resp_headers.push((":status".into(), status.as_u16().to_string()));
        }
        resp_headers.extend(header_list(&parts.headers));
        let kind = classify(o.method.as_str(), &o.path, req_ct.as_deref(), resp_ct.as_deref(), is_upgrade && status == StatusCode::SWITCHING_PROTOCOLS);
        let rv = version_str(parts.version).to_string();
        sh.store.update(id, |f| {
            f.summary.status = Some(status.as_u16());
            f.summary.kind = kind.into();
            f.summary.content_type = resp_ct.clone();
            f.summary.resp_header_count = resp_headers.len();
            f.status_text = status.canonical_reason().map(|s| s.to_string());
            f.resp_http_version = Some(rv);
            f.resp_headers = resp_headers;
            f.timing.waiting_ms = Some(waiting_ms);
        });
    }

    // WebSocket and other upgrades: splice the two upgraded connections together.
    if let (Some(client_up), Some(server_up)) = (on_upgrade, server_up) {
        {
            let sh2 = sh.clone();
            let started = o.started;
            if let Some(id) = id {
                sh.store.update(id, |f| {
                    f.summary.state = FlowState::Live;
                    f.summary.note = Some("live".into());
                });
            }
            tokio::spawn(async move {
                let res = async {
                    let (c, s) = tokio::try_join!(client_up, server_up)?;
                    let (mut c, mut s) = (TokioIo::new(c), TokioIo::new(s));
                    Ok::<_, BoxError>(tokio::io::copy_bidirectional(&mut c, &mut s).await?)
                }
                .await;
                if let Some(id) = id {
                    let ms = started.elapsed().as_secs_f64() * 1000.0;
                    sh2.store.update(id, |f| {
                        f.summary.duration_ms = Some(ms);
                        f.timing.total_ms = Some(ms);
                        f.summary.note = None;
                        match res {
                            Ok((up, down)) => {
                                f.summary.req_size = up;
                                f.summary.resp_size = down;
                                f.summary.state = FlowState::Complete;
                            }
                            Err(e) => {
                                f.summary.state = FlowState::Complete;
                                f.summary.error = Some(e.to_string());
                            }
                        }
                    });
                }
            });
            strip_hop_by_hop(&mut parts.headers, true);
            return Response::from_parts(parts, full(Bytes::new()));
        }
    }

    strip_hop_by_hop(&mut parts.headers, false);
    let body: RespBody = match id {
        Some(id) => {
            let sh2 = sh.clone();
            let started = o.started;
            TeeBody::new(body, sh.body_cap, move |d: TeeDone| {
                let total_ms = started.elapsed().as_secs_f64() * 1000.0;
                sh2.store.update(id, |f| {
                    f.resp_body = d.captured;
                    f.resp_body_truncated = d.truncated;
                    f.summary.resp_size = d.total;
                    f.summary.duration_ms = Some(total_ms);
                    f.timing.download_ms = Some(d.elapsed_ms);
                    f.timing.total_ms = Some(total_ms);
                    match d.error {
                        Some(e) => {
                            f.summary.state = FlowState::Error;
                            f.summary.error = Some(e);
                        }
                        None => f.summary.state = FlowState::Complete,
                    }
                });
            })
            .map_err(|e| Box::new(e) as BoxError)
            .boxed_unsync()
        }
        None => body.map_err(|e| Box::new(e) as BoxError).boxed_unsync(),
    };
    Response::from_parts(parts, body)
}

fn fail(sh: &Shared, id: Option<u64>, o: &Outgoing, status: StatusCode, msg: String) -> Response<RespBody> {
    tracing::debug!("{} {}: {msg}", o.method, o.host);
    if let Some(id) = id {
        let ms = o.started.elapsed().as_secs_f64() * 1000.0;
        let m = msg.clone();
        sh.store.update(id, |f| {
            f.summary.state = FlowState::Error;
            f.summary.error = Some(m);
            f.summary.duration_ms = Some(ms);
            f.timing.total_ms = Some(ms);
        });
    }
    let mut r = text_response(status, format!("xwire could not reach {}:{}\n\n{msg}\n", o.host, o.port));
    r.headers_mut().insert("x-xwire-error", HeaderValue::from_static("1"));
    r
}

fn special_host(sh: &Shared, host: &str) -> Option<Response<RespBody>> {
    match host {
        // `curl https://xwire.check` through the proxy proves interception works end to end
        "xwire.check" => {
            let mut r = Response::new(full(r#"{"ok":true,"proxy":"xwire"}"#));
            r.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json"));
            Some(r)
        }
        "xwire.cert" | "xwire.ssl" => Some(cert_download(sh, "pem")),
        _ => None,
    }
}

fn cert_download(sh: &Shared, kind: &str) -> Response<RespBody> {
    let (body, ct, name): (Bytes, &str, &str) = match kind {
        "der" => (Bytes::copy_from_slice(sh.ca.der()), "application/x-x509-ca-cert", "xwire-root.der"),
        _ => (Bytes::from(sh.ca.pem().to_string()), "application/x-x509-ca-cert", "xwire-root.pem"),
    };
    let mut r = Response::new(full(body));
    r.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static(ct));
    if let Ok(v) = HeaderValue::from_str(&format!("attachment; filename=\"{name}\"")) {
        r.headers_mut().insert(header::CONTENT_DISPOSITION, v);
    }
    r
}

fn local_page<B>(sh: &Shared, req: &Request<B>) -> Response<RespBody> {
    match req.uri().path() {
        "/cert" | "/cert.pem" | "/ssl" => return cert_download(sh, "pem"),
        "/cert.der" | "/cert.crt" => return cert_download(sh, "der"),
        _ => {}
    }
    let html = format!(
        "<!doctype html><meta name=viewport content='width=device-width'><title>xwire</title>\
         <body style='font:16px system-ui;background:#0a0c0f;color:#e6e9ee;padding:32px;max-width:560px;margin:auto'>\
         <h2>xwire is running</h2><p>This device is talking to the xwire proxy at <code>{}</code>.</p>\
         <p><a style='color:#8b93ff' href='/cert.pem'>Download the xwire root certificate (.pem)</a><br>\
         <a style='color:#8b93ff' href='/cert.der'>Download as .crt (DER, Android)</a></p>\
         <p style='color:#a1aab6'>Fingerprint (SHA-256): <code style='word-break:break-all'>{}</code></p></body>",
        sh.local,
        sh.ca.info().sha256
    );
    let mut r = Response::new(full(html));
    r.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"));
    r
}

/// Decode a captured body for display (undoes Content-Encoding).
pub fn decoded_body(body: &[u8], headers: &Headers) -> (Vec<u8>, Option<String>) {
    let enc = headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("content-encoding")).map(|(_, v)| v.as_str());
    match decode(body, enc) {
        Ok(b) => (b, None),
        Err(e) => (body.to_vec(), Some(e)),
    }
}

pin_project_lite::pin_project! {
    /// A stream that yields `pre` before reading from `inner` (used after peeking the first byte of a tunnel).
    pub struct Rewind<S> {
        pre: Option<Bytes>,
        #[pin]
        inner: S,
    }
}

impl<S> Rewind<S> {
    pub fn new(pre: Bytes, inner: S) -> Self {
        Self { pre: if pre.is_empty() { None } else { Some(pre) }, inner }
    }
}

impl<S: AsyncRead> AsyncRead for Rewind<S> {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        let this = self.project();
        if let Some(mut pre) = this.pre.take() {
            let n = pre.len().min(buf.remaining());
            buf.put_slice(&pre.split_to(n));
            if !pre.is_empty() {
                *this.pre = Some(pre);
            }
            return Poll::Ready(Ok(()));
        }
        this.inner.poll_read(cx, buf)
    }
}

impl<S: AsyncWrite> AsyncWrite for Rewind<S> {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<std::io::Result<usize>> {
        self.project().inner.poll_write(cx, buf)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        self.project().inner.poll_flush(cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        self.project().inner.poll_shutdown(cx)
    }
    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<std::io::Result<usize>> {
        self.project().inner.poll_write_vectored(cx, bufs)
    }
    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globs() {
        assert!(glob_match("*.apple.com", "api.apple.com"));
        assert!(glob_match("*.apple.com", "apple.com"));
        assert!(!glob_match("*.apple.com", "notapple.com"));
        assert!(glob_match("accounts.vendor.dev", "ACCOUNTS.vendor.dev"));
        assert!(glob_match("*bank*", "my.bank.example"));
    }

    #[test]
    fn host_port() {
        assert_eq!(split_host_port("example.com:8443", 443), ("example.com".into(), 8443));
        assert_eq!(split_host_port("example.com", 443), ("example.com".into(), 443));
        assert_eq!(split_host_port("[::1]:9000", 443), ("::1".into(), 9000));
    }
}
