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
    if let Some(rest) = p.strip_prefix("*.")
        && t == rest {
            return true;
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
    /// Shared with the app so rules survive proxy restarts.
    pub rules: crate::rules::SharedRules,
    pub breaks: Arc<crate::rules::Breaks>,
}

impl Default for ProxyOptions {
    fn default() -> Self {
        Self {
            listen: "127.0.0.1:9090".parse().unwrap(),
            intercept: InterceptConfig::default(),
            body_cap: 16 << 20,
            rules: Default::default(),
            breaks: Default::default(),
        }
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
    rules: crate::rules::SharedRules,
    breaks: Arc<crate::rules::Breaks>,
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
            rules: opts.rules,
            breaks: opts.breaks,
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
        tracing::info!("WireCraft proxy listening on {local}");
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
    pub fn replay(&self, id: u64) -> Result<u64> {
        let f = self.shared.store.get(id).ok_or_else(|| anyhow::anyhow!("flow {id} no longer exists"))?;
        let s = &f.summary;
        let default = if s.scheme == "https" { 443 } else { 80 };
        let auth = if s.port == default { s.host.clone() } else { format!("{}:{}", s.host, s.port) };
        let url = format!("{}://{}{}", s.scheme, auth, s.path);
        self.send(&s.method, &url, &f.req_headers, f.req_body.clone(), "WireCraft · repeat")
    }

    /// Send a new request through the proxy pipeline (rules apply). Returns the new flow id immediately; the
    /// exchange continues in the background.
    pub fn send(&self, method: &str, url: &str, headers: &Headers, body: Bytes, client: &str) -> Result<u64> {
        let method = Method::from_bytes(method.trim().to_ascii_uppercase().as_bytes())?;
        let mut out = Outgoing {
            method,
            scheme: "http",
            host: String::new(),
            port: 80,
            path: "/".into(),
            version: "HTTP/1.1".into(),
            headers: headers_from_list(headers),
            body,
            client: Some(client.to_string()),
            client_addr: "wirecraft".into(),
            tls: None,
            started: Instant::now(),
            started_unix: now_ms(),
            request_ms: 0.0,
        };
        retarget(&mut out, url.trim()).map_err(|e| anyhow::anyhow!(e))?;
        if !out.body.is_empty() || out.headers.contains_key(header::CONTENT_LENGTH) {
            out.headers.insert(header::CONTENT_LENGTH, HeaderValue::from(out.body.len()));
        }
        let new_id = self.shared.store.next_id();
        let sh = self.shared.clone();
        tokio::spawn(async move {
            let resp = exchange(sh, out, None, Some(new_id)).await;
            // drain so the tee records the body and closes the flow
            let _ = resp.into_body().collect().await;
        });
        Ok(new_id)
    }

    pub fn rules(&self) -> Arc<crate::rules::Rules> {
        self.shared.rules.read().clone()
    }

    pub fn breaks(&self) -> &Arc<crate::rules::Breaks> {
        &self.shared.breaks
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
    // SOCKS5 shares the port: its greeting starts with the version byte 0x05, which no HTTP request does.
    let mut first = [0u8; 1];
    if matches!(stream.peek(&mut first).await, Ok(1)) && first[0] == 0x05 {
        return serve_socks5(sh, conn, stream).await;
    }
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

/// SOCKS5 (RFC 1928), no authentication, CONNECT only. The tunnel is then handled exactly like an HTTP
/// CONNECT: decrypted when interception applies to the host, passed through otherwise.
async fn serve_socks5(sh: Arc<Shared>, conn: Conn, mut s: TcpStream) {
    use tokio::io::AsyncWriteExt;
    let target = async {
        let mut hdr = [0u8; 2];
        s.read_exact(&mut hdr).await?;
        let mut methods = vec![0u8; hdr[1] as usize];
        s.read_exact(&mut methods).await?;
        if !methods.contains(&0x00) {
            s.write_all(&[0x05, 0xff]).await?;
            return Err(std::io::Error::other("client offers no supported SOCKS auth method"));
        }
        s.write_all(&[0x05, 0x00]).await?;
        let mut req = [0u8; 4];
        s.read_exact(&mut req).await?;
        let host = match req[3] {
            0x01 => {
                let mut a = [0u8; 4];
                s.read_exact(&mut a).await?;
                std::net::Ipv4Addr::from(a).to_string()
            }
            0x03 => {
                let mut n = [0u8; 1];
                s.read_exact(&mut n).await?;
                let mut name = vec![0u8; n[0] as usize];
                s.read_exact(&mut name).await?;
                String::from_utf8_lossy(&name).into_owned()
            }
            0x04 => {
                let mut a = [0u8; 16];
                s.read_exact(&mut a).await?;
                std::net::Ipv6Addr::from(a).to_string()
            }
            _ => {
                s.write_all(&[0x05, 0x08, 0x00, 0x01, 0, 0, 0, 0, 0, 0]).await?;
                return Err(std::io::Error::other("unsupported SOCKS address type"));
            }
        };
        let mut port = [0u8; 2];
        s.read_exact(&mut port).await?;
        let port = u16::from_be_bytes(port);
        if req[0] != 0x05 || req[1] != 0x01 {
            // only CONNECT; BIND and UDP ASSOCIATE are refused
            s.write_all(&[0x05, 0x07, 0x00, 0x01, 0, 0, 0, 0, 0, 0]).await?;
            return Err(std::io::Error::other("unsupported SOCKS command"));
        }
        s.write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0]).await?;
        Ok((host, port))
    }
    .await;
    let (host, port) = match target {
        Ok(t) => t,
        Err(e) => {
            tracing::debug!("SOCKS5 {}: {e}", conn.peer);
            return;
        }
    };
    if sh.intercept.read().should_decrypt(&host) {
        intercept_tunnel(sh, conn, s, host, port, None).await;
    } else {
        blind_tunnel(sh, conn, s, host, port, None, Some("SOCKS5 · tunnelled".into())).await;
    }
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
            applied: vec![],
            ws_messages: 0,
        },
        req_headers: vec![("host".into(), format!("{host}:{port}"))],
        req_body: Bytes::new(),
        req_body_truncated: false,
        status_text: Some("Connection Established".into()),
        resp_http_version: None,
        resp_headers: vec![],
        resp_body: Bytes::new(),
        resp_body_truncated: false,
            resp_trailers: vec![],
        timing: Timing { started, ..Default::default() },
        tls: None,
        ws: vec![],
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
    match tokio::time::timeout(Duration::from_secs(2), io.read(&mut first)).await {
        Ok(Ok(0)) | Ok(Err(_)) => return,
        Ok(Ok(_)) => {}
        // the server talks first (SMTP, IMAP, SSH, …): nothing to decrypt
        Err(_) => return blind_tunnel(sh, conn, io, host, port, ua, Some("server speaks first".into())).await,
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
            "TLS handshake failed: {err}. The client doesn't trust the WireCraft root CA, or it pins certificates \
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
        Err(e) => return text_response(StatusCode::BAD_REQUEST, format!("WireCraft: reading request body: {e}")),
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

fn headers_from_list(list: &Headers) -> HeaderMap {
    let mut h = HeaderMap::new();
    for (k, v) in list {
        if k.starts_with(':') {
            continue;
        }
        if let (Ok(k), Ok(v)) = (HeaderName::from_bytes(k.trim().as_bytes()), HeaderValue::from_str(v)) {
            h.append(k, v);
        }
    }
    h
}

/// Point `o` at a new absolute URL.
fn retarget(o: &mut Outgoing, url: &str) -> Result<(), String> {
    let uri: Uri = url.parse().map_err(|e| format!("invalid URL {url}: {e}"))?;
    let https = match uri.scheme_str() {
        Some(s) if s.eq_ignore_ascii_case("https") => true,
        Some(s) if s.eq_ignore_ascii_case("http") => false,
        _ => return Err(format!("URL must start with http:// or https://: {url}")),
    };
    let auth = uri.authority().ok_or_else(|| format!("URL has no host: {url}"))?;
    let (h, p) = split_host_port(auth.as_str(), if https { 443 } else { 80 });
    o.scheme = if https { "https" } else { "http" };
    o.host = h;
    o.port = p;
    o.path = uri.path_and_query().map(|p| p.as_str().to_string()).unwrap_or_else(|| "/".into());
    Ok(())
}

/// The Type column: `classify`, plus GraphQL recognised from the request body when the path doesn't say so.
fn kind_of(kind: &'static str, o: &Outgoing) -> &'static str {
    if matches!(kind, "JSON" | "—") && crate::body::looks_like_graphql(o.method.as_str(), &o.body) { "GQL" } else { kind }
}

fn script_request(o: &Outgoing) -> crate::scripts::ScriptRequest {
    crate::scripts::ScriptRequest {
        method: o.method.to_string(),
        url: display_url(o),
        headers: header_list(&o.headers),
        body: o.body.to_vec(),
    }
}

fn record_request(sh: &Shared, id: u64, o: &Outgoing) {
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
    let body = if truncated { o.body.slice(..cap) } else { o.body.clone() };
    let n = req_headers.len();
    let (m, sc, host, port, path) = (o.method.to_string(), o.scheme, o.host.clone(), o.port, o.path.clone());
    let len = o.body.len() as u64;
    sh.store.update(id, |f| {
        f.summary.method = m;
        f.summary.scheme = sc.into();
        f.summary.tls = sc == "https";
        f.summary.host = host;
        f.summary.port = port;
        f.summary.path = path;
        f.summary.req_size = len;
        f.summary.req_header_count = n;
        f.req_headers = req_headers;
        f.req_body = body;
        f.req_body_truncated = truncated;
    });
}

fn mark(sh: &Shared, id: Option<u64>, label: &str, note: Option<String>) {
    if let Some(id) = id {
        let label = label.to_string();
        sh.store.update(id, |f| {
            if !f.summary.applied.contains(&label) {
                f.summary.applied.push(label);
            }
            if note.is_some() {
                f.summary.note = note;
            }
        });
    }
}

const RULE_BODY_CAP: usize = 32 << 20;

/// Record the request, run it through the rules, forward it upstream (or answer it locally) and return the
/// response for the client (body tee'd into the store).
async fn exchange(sh: Arc<Shared>, mut o: Outgoing, on_upgrade: Option<hyper::upgrade::OnUpgrade>, id: Option<u64>) -> Response<RespBody> {
    sh.requests.fetch_add(1, Ordering::Relaxed);
    let rules = sh.rules.read().clone();
    let original_url = display_url(&o);
    let original_method = o.method.to_string();
    let req_ct = o.headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).map(|s| s.to_string());
    let is_upgrade = on_upgrade.is_some();
    let xhr = o.headers.get("sec-fetch-dest").map(|v| v.as_bytes() == b"empty").unwrap_or(false)
        || o.headers.contains_key("x-requested-with");
    let rewrites: Vec<crate::rules::Rewrite> = rules.rewrites_for(&original_method, &original_url).cloned().collect();

    // ---- request-side rewrites
    let mut rewrote = false;
    for r in rewrites.iter().filter(|r| !r.target.is_response()) {
        use crate::rules::RewriteTarget as T;
        match r.target {
            T::Url | T::QueryParam => {
                let u = r.apply_url(&display_url(&o));
                if retarget(&mut o, &u).is_ok() {
                    rewrote = true;
                }
            }
            T::RequestHeader => {
                let mut list = header_list(&o.headers);
                r.apply_headers(&mut list);
                o.headers = headers_from_list(&list);
                rewrote = true;
            }
            T::RequestBody => {
                o.body = Bytes::from(r.apply_body(&o.body));
                if o.headers.contains_key(header::CONTENT_LENGTH) {
                    o.headers.insert(header::CONTENT_LENGTH, HeaderValue::from(o.body.len()));
                }
                rewrote = true;
            }
            _ => {}
        }
    }

    if let Some(id) = id {
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
                kind: kind_of(classify(o.method.as_str(), &o.path, req_ct.as_deref(), None, is_upgrade), &o).into(),
                content_type: None,
                req_size: 0,
                resp_size: 0,
                started: o.started_unix,
                duration_ms: None,
                client: o.client.clone(),
                client_addr: o.client_addr.clone(),
                tls: o.scheme == "https",
                xhr,
                error: None,
                note: None,
                req_header_count: 0,
                resp_header_count: 0,
                applied: if rewrote { vec!["Rewrite".into()] } else { vec![] },
                ws_messages: 0,
            },
            req_headers: vec![],
            req_body: Bytes::new(),
            req_body_truncated: false,
            status_text: None,
            resp_http_version: None,
            resp_headers: vec![],
            resp_body: Bytes::new(),
            resp_body_truncated: false,
            resp_trailers: vec![],
            timing: Timing { started: o.started_unix, request_ms: Some(o.request_ms), ..Default::default() },
            tls: o.tls.clone(),
            ws: vec![],
        });
        record_request(&sh, id, &o);
    }

    // ---- block
    if rules.blocked(&original_method, &original_url) {
        mark(&sh, id, "Blocked", Some("blocked".into()));
        let mut r = text_response(StatusCode::FORBIDDEN, "Blocked by WireCraft\n");
        r.headers_mut().insert("x-wirecraft-blocked", HeaderValue::from_static("1"));
        return finish_local(&sh, id, &o, r);
    }

    // ---- request scripts
    let req_scripts: Vec<crate::scripts::Script> = rules.scripts_for(&original_method, &original_url, false).cloned().collect();
    for script in req_scripts {
        use crate::scripts::RequestOutcome;
        let sreq = script_request(&o);
        let (client, s2, sreq2) = (o.client.clone(), script.clone(), sreq.clone());
        let res = tokio::task::spawn_blocking(move || crate::scripts::run_request(&s2, id, client.as_deref(), &sreq2))
            .await
            .unwrap_or_else(|e| Err(format!("script panicked: {e}")));
        match res {
            Ok(Some(RequestOutcome::Continue(r))) => {
                if r == sreq {
                    continue;
                }
                if r.method != sreq.method {
                    match Method::from_bytes(r.method.as_bytes()) {
                        Ok(m) => o.method = m,
                        Err(_) => return fail(&sh, id, &o, StatusCode::BAD_REQUEST, format!("{}: invalid method {}", script.label(), r.method)),
                    }
                }
                if r.url != sreq.url
                    && let Err(e) = retarget(&mut o, &r.url) {
                        return fail(&sh, id, &o, StatusCode::BAD_REQUEST, format!("{}: {e}", script.label()));
                    }
                if r.headers != sreq.headers {
                    o.headers = headers_from_list(&r.headers);
                }
                if r.body != sreq.body || r.headers != sreq.headers {
                    o.body = Bytes::from(r.body);
                    if o.headers.contains_key(header::CONTENT_LENGTH) || !o.body.is_empty() {
                        o.headers.insert(header::CONTENT_LENGTH, HeaderValue::from(o.body.len()));
                    }
                }
                mark(&sh, id, "Script", Some(script.label()));
                if let Some(fid) = id {
                    record_request(&sh, fid, &o);
                }
            }
            Ok(Some(RequestOutcome::Respond(r))) => {
                let body = Bytes::from(r.body);
                let mut resp = Response::new(full(body.clone()));
                *resp.status_mut() = StatusCode::from_u16(r.status).unwrap_or(StatusCode::OK);
                *resp.headers_mut() = headers_from_list(&r.headers);
                resp.headers_mut().remove(header::TRANSFER_ENCODING);
                resp.headers_mut().remove(header::CONTENT_ENCODING);
                resp.headers_mut().insert(header::CONTENT_LENGTH, HeaderValue::from(body.len()));
                resp.headers_mut().insert("x-wirecraft-script", HeaderValue::from_static("1"));
                mark(&sh, id, "Script", Some(format!("{} · answered", script.label())));
                return respond(sh, o, id, resp, None, None, rules, rewrites, req_ct, 0.0).await;
            }
            Ok(None) => {}
            Err(e) => mark(&sh, id, "Script error", Some(format!("{}: {e}", script.label()))),
        }
    }

    // ---- request breakpoint
    if let (Some(fid), Some(_)) = (id, rules.breakpoint_for(&original_method, &original_url, false)) {
        let paused = crate::rules::Paused {
            flow_id: fid,
            phase: "request",
            method: o.method.to_string(),
            url: display_url(&o),
            status: None,
            headers: header_list(&o.headers),
            body: String::from_utf8_lossy(&o.body).into_owned(),
            binary: std::str::from_utf8(&o.body).is_err(),
        };
        let rx = sh.breaks.pause(paused);
        sh.store.update(fid, |f| {
            f.summary.state = FlowState::Paused;
            f.summary.note = Some("paused · request".into());
        });
        let r = rx.await.unwrap_or_default();
        sh.store.update(fid, |f| {
            f.summary.state = FlowState::Pending;
            f.summary.note = None;
        });
        mark(&sh, id, "Breakpoint", None);
        if r.abort {
            return fail(&sh, id, &o, StatusCode::BAD_GATEWAY, "aborted at a request breakpoint".into());
        }
        if let Some(m) = r.method.as_deref().filter(|m| !m.trim().is_empty())
            && let Ok(m) = Method::from_bytes(m.trim().to_ascii_uppercase().as_bytes()) {
                o.method = m;
            }
        if let Some(u) = r.url.as_deref().filter(|u| !u.trim().is_empty())
            && let Err(e) = retarget(&mut o, u.trim()) {
                return fail(&sh, id, &o, StatusCode::BAD_REQUEST, e);
            }
        if let Some(h) = &r.headers {
            o.headers = headers_from_list(h);
        }
        if let Some(b) = r.body {
            o.body = Bytes::from(b);
            if o.headers.contains_key(header::CONTENT_LENGTH) || !o.body.is_empty() {
                o.headers.insert(header::CONTENT_LENGTH, HeaderValue::from(o.body.len()));
            }
        }
        record_request(&sh, fid, &o);
    }
    o.started = o.started.max(Instant::now() - Duration::from_millis(o.request_ms as u64));

    // ---- map local
    if let Some(ml) = rules.map_local_for(&original_method, &original_url).cloned() {
        let body = if ml.file.trim().is_empty() {
            Ok(Bytes::from(ml.body.clone()))
        } else {
            tokio::fs::read(ml.file.trim()).await.map(Bytes::from).map_err(|e| format!("Map Local file {}: {e}", ml.file))
        };
        let body = match body {
            Ok(b) => b,
            Err(e) => return fail(&sh, id, &o, StatusCode::INTERNAL_SERVER_ERROR, e),
        };
        let mut r = Response::new(full(body.clone()));
        *r.status_mut() = StatusCode::from_u16(ml.status).unwrap_or(StatusCode::OK);
        *r.headers_mut() = headers_from_list(&ml.headers);
        r.headers_mut().insert(header::CONTENT_LENGTH, HeaderValue::from(body.len()));
        r.headers_mut().insert("x-wirecraft-map-local", HeaderValue::from_static("1"));
        let label = if ml.name.trim().is_empty() { "Map Local".to_string() } else { format!("Map Local · {}", ml.name.trim()) };
        mark(&sh, id, "Map Local", Some(label));
        return respond(sh, o, id, r, None, None, rules, rewrites, req_ct, 0.0).await;
    }

    // ---- map remote
    if let Some(mr) = rules.map_remote_for(&original_method, &original_url)
        && let Some(u) = crate::rules::map_remote_url(mr, &display_url(&o)) {
            if let Err(e) = retarget(&mut o, &u) {
                return fail(&sh, id, &o, StatusCode::BAD_REQUEST, e);
            }
            mark(&sh, id, "Map Remote", Some(format!("→ {u}")));
        }

    // ---- throttle (latency + upload)
    let throttle = rules.throttle.applies(&o.host).then(|| rules.throttle.clone());
    if let Some(t) = &throttle {
        mark(&sh, id, "Throttle", None);
        tokio::time::sleep(crate::rules::upload_delay(t, o.body.len())).await;
    }

    // ---- upstream
    let url = display_url(&o);
    let uri: Uri = match url.parse() {
        Ok(u) => u,
        Err(e) => return fail(&sh, id, &o, StatusCode::BAD_REQUEST, format!("invalid URL {url}: {e}")),
    };
    let mut headers = o.headers.clone();
    strip_hop_by_hop(&mut headers, is_upgrade);
    if is_upgrade {
        // keep WebSocket payloads readable: no permessage-deflate
        headers.remove("sec-websocket-extensions");
    }
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
    let server_up = (resp.status() == StatusCode::SWITCHING_PROTOCOLS && on_upgrade.is_some()).then(|| hyper::upgrade::on(&mut resp));
    let resp = resp.map(|b| b.map_err(|e| Box::new(e) as BoxError).boxed_unsync());
    let upgrade = match (on_upgrade, server_up) {
        (Some(c), Some(s)) => Some((c, s)),
        _ => None,
    };
    respond(sh, o, id, resp, upgrade, throttle, rules, rewrites, req_ct, waiting_ms).await
}

/// A response produced by WireCraft itself (blocked): record it and send it.
fn finish_local(sh: &Shared, id: Option<u64>, o: &Outgoing, r: Response<RespBody>) -> Response<RespBody> {
    if let Some(id) = id {
        let status = r.status();
        let headers = header_list(r.headers());
        let ms = o.started.elapsed().as_secs_f64() * 1000.0;
        sh.store.update(id, |f| {
            f.summary.status = Some(status.as_u16());
            f.summary.state = FlowState::Complete;
            f.summary.duration_ms = Some(ms);
            f.summary.resp_header_count = headers.len();
            f.status_text = status.canonical_reason().map(|s| s.to_string());
            f.resp_headers = headers;
            f.resp_body = Bytes::from_static(b"Blocked by WireCraft\n");
            f.summary.resp_size = f.resp_body.len() as u64;
            f.timing.total_ms = Some(ms);
        });
    }
    r
}

/// Response phase: response rules, breakpoint, recording, throttling, upgrades.
#[allow(clippy::too_many_arguments)]
async fn respond(
    sh: Arc<Shared>,
    o: Outgoing,
    id: Option<u64>,
    resp: Response<RespBody>,
    upgrade: Option<(hyper::upgrade::OnUpgrade, hyper::upgrade::OnUpgrade)>,
    throttle: Option<crate::rules::Throttle>,
    rules: Arc<crate::rules::Rules>,
    rewrites: Vec<crate::rules::Rewrite>,
    req_ct: Option<String>,
    waiting_ms: f64,
) -> Response<RespBody> {
    let (mut parts, mut body) = resp.into_parts();
    let url = display_url(&o);
    let method = o.method.to_string();

    // ---- response rewrites and breakpoint need the whole body
    let resp_rewrites: Vec<crate::rules::Rewrite> = rewrites.into_iter().filter(|r| r.target.is_response()).collect();
    let bp = id.is_some() && rules.breakpoint_for(&method, &url, true).is_some();
    let resp_scripts: Vec<crate::scripts::Script> = rules.scripts_for(&method, &url, true).cloned().collect();
    drop(rules);
    if upgrade.is_none() && (!resp_rewrites.is_empty() || bp || !resp_scripts.is_empty()) {
        let bytes = match crate::rules::collect_capped(body, RULE_BODY_CAP).await {
            Ok(Some(b)) => b,
            Ok(None) => return fail(&sh, id, &o, StatusCode::BAD_GATEWAY, "response too large for rewrite / breakpoint (32 MB)".into()),
            Err(e) => return fail(&sh, id, &o, StatusCode::BAD_GATEWAY, format!("reading response: {e}")),
        };
        // work on the decoded body so text edits make sense
        let enc = parts.headers.get(header::CONTENT_ENCODING).and_then(|v| v.to_str().ok()).map(|s| s.to_string());
        let mut data = match &enc {
            Some(e) => match decode(&bytes, Some(e)) {
                Ok(d) => {
                    parts.headers.remove(header::CONTENT_ENCODING);
                    d
                }
                Err(_) => bytes.to_vec(),
            },
            None => bytes.to_vec(),
        };
        let mut status = parts.status.as_u16();
        let mut hl = header_list(&parts.headers);
        for r in &resp_rewrites {
            use crate::rules::RewriteTarget as T;
            match r.target {
                T::Status => status = r.apply_status(status),
                T::ResponseHeader => r.apply_headers(&mut hl),
                T::ResponseBody => data = r.apply_body(&data),
                _ => {}
            }
        }
        if !resp_rewrites.is_empty() {
            mark(&sh, id, "Rewrite", None);
        }
        if !resp_scripts.is_empty() {
            let sreq = script_request(&o);
            for script in resp_scripts {
                let sresp = crate::scripts::ScriptResponse { status, headers: hl.clone(), body: std::mem::take(&mut data) };
                let (client, s2, q, r) = (o.client.clone(), script.clone(), sreq.clone(), sresp.clone());
                let res = tokio::task::spawn_blocking(move || crate::scripts::run_response(&s2, id, client.as_deref(), &q, &r))
                    .await
                    .unwrap_or_else(|e| Err(format!("script panicked: {e}")));
                match res {
                    Ok(Some(r)) => {
                        if r != sresp {
                            mark(&sh, id, "Script", Some(script.label()));
                        }
                        status = r.status;
                        hl = r.headers;
                        data = r.body;
                    }
                    Ok(None) => data = sresp.body,
                    Err(e) => {
                        data = sresp.body;
                        mark(&sh, id, "Script error", Some(format!("{}: {e}", script.label())));
                    }
                }
            }
            hl.retain(|(k, _)| !k.eq_ignore_ascii_case("content-encoding"));
        }
        if let (Some(fid), true) = (id, bp) {
            let paused = crate::rules::Paused {
                flow_id: fid,
                phase: "response",
                method: method.clone(),
                url: url.clone(),
                status: Some(status),
                headers: hl.clone(),
                body: String::from_utf8_lossy(&data).into_owned(),
                binary: std::str::from_utf8(&data).is_err(),
            };
            let rx = sh.breaks.pause(paused);
            sh.store.update(fid, |f| {
                f.summary.state = FlowState::Paused;
                f.summary.status = Some(status);
                f.summary.note = Some("paused · response".into());
            });
            let r = rx.await.unwrap_or_default();
            sh.store.update(fid, |f| {
                f.summary.state = FlowState::Pending;
                f.summary.note = None;
            });
            mark(&sh, id, "Breakpoint", None);
            if r.abort {
                return fail(&sh, id, &o, StatusCode::BAD_GATEWAY, "aborted at a response breakpoint".into());
            }
            if let Some(s) = r.status {
                status = s;
            }
            if let Some(h) = r.headers {
                hl = h;
            }
            if let Some(b) = r.body {
                data = b.into_bytes();
            }
        }
        parts.status = StatusCode::from_u16(status).unwrap_or(parts.status);
        parts.headers = headers_from_list(&hl);
        parts.headers.remove(header::TRANSFER_ENCODING);
        parts.headers.insert(header::CONTENT_LENGTH, HeaderValue::from(data.len()));
        body = full(data);
    }

    let status = parts.status;
    let resp_ct = parts.headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).map(|s| s.to_string());
    if let Some(id) = id {
        let mut resp_headers = Vec::new();
        if parts.version == Version::HTTP_2 {
            resp_headers.push((":status".into(), status.as_u16().to_string()));
        }
        resp_headers.extend(header_list(&parts.headers));
        let kind = kind_of(classify(o.method.as_str(), &o.path, req_ct.as_deref(), resp_ct.as_deref(), upgrade.is_some() && status == StatusCode::SWITCHING_PROTOCOLS), &o);
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

    // WebSocket and other upgrades: splice the two upgraded connections together, capturing WS messages.
    if let Some((client_up, server_up)) = upgrade {
        let sh2 = sh.clone();
        let started = o.started;
        let ws = parts.headers.get(header::UPGRADE).map(|v| v.as_bytes().eq_ignore_ascii_case(b"websocket")).unwrap_or(false);
        if let Some(id) = id {
            sh.store.update(id, |f| {
                f.summary.state = FlowState::Live;
                f.summary.note = Some("live".into());
            });
        }
        tokio::spawn(async move {
            let res = async {
                let (c, s) = tokio::try_join!(client_up, server_up)?;
                let (c, s) = (TokioIo::new(c), TokioIo::new(s));
                if let (true, Some(fid)) = (ws, id) {
                    let (cr, cw) = tokio::io::split(c);
                    let (sr, sw) = tokio::io::split(s);
                    let rec = |sh: Arc<Shared>| {
                        move |msgs: Vec<crate::store::WsMessage>| {
                            sh.store.update(fid, |f| {
                                f.summary.ws_messages += msgs.len() as u64;
                                f.summary.note = Some(format!("{} messages", f.summary.ws_messages));
                                for m in msgs {
                                    if f.ws.len() < crate::ws::MAX_MESSAGES {
                                        f.ws.push(m);
                                    }
                                }
                            });
                        }
                    };
                    let up = crate::ws::pump(cr, sw, crate::ws::Parser::new(true), rec(sh2.clone()));
                    let down = crate::ws::pump(sr, cw, crate::ws::Parser::new(false), rec(sh2.clone()));
                    let (u, d) = tokio::join!(up, down);
                    // one side closing ends the other with an error; report what was copied
                    Ok::<_, BoxError>((u.unwrap_or(0), d.unwrap_or(0)))
                } else {
                    let (mut c, mut s) = (c, s);
                    Ok::<_, BoxError>(tokio::io::copy_bidirectional(&mut c, &mut s).await?)
                }
            }
            .await;
            if let Some(id) = id {
                let ms = started.elapsed().as_secs_f64() * 1000.0;
                sh2.store.update(id, |f| {
                    f.summary.duration_ms = Some(ms);
                    f.timing.total_ms = Some(ms);
                    f.summary.note = (f.summary.ws_messages > 0).then(|| format!("{} messages", f.summary.ws_messages));
                    f.summary.state = FlowState::Complete;
                    match res {
                        Ok((up, down)) => {
                            f.summary.req_size = up;
                            f.summary.resp_size = down;
                        }
                        Err(e) => {
                            let e = e.to_string();
                            // a close without TLS close_notify is how most WebSocket servers hang up
                            if !e.contains("close_notify") {
                                f.summary.error = Some(e);
                            }
                        }
                    }
                });
            }
        });
        strip_hop_by_hop(&mut parts.headers, true);
        return Response::from_parts(parts, full(Bytes::new()));
    }

    strip_hop_by_hop(&mut parts.headers, false);
    let body: RespBody = match throttle.as_ref().filter(|t| t.download_kbps > 0) {
        Some(t) => crate::rules::Throttled::new(body, t.download_kbps).boxed_unsync(),
        None => body,
    };
    let body: RespBody = match id {
        Some(id) => {
            let sh2 = sh.clone();
            let started = o.started;
            let expected = parts.headers.get(header::CONTENT_LENGTH).and_then(|v| v.to_str().ok()).and_then(|v| v.trim().parse().ok());
            TeeBody::new(body, sh.body_cap, move |d: TeeDone| {
                let total_ms = started.elapsed().as_secs_f64() * 1000.0;
                sh2.store.update(id, |f| {
                    f.resp_body = d.captured;
                    f.resp_body_truncated = d.truncated;
                    f.resp_trailers = d.trailers;
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
            .expect_len(expected)
            .boxed_unsync()
        }
        None => body,
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
    let mut r = text_response(status, format!("WireCraft could not reach {}:{}\n\n{msg}\n", o.host, o.port));
    r.headers_mut().insert("x-wirecraft-error", HeaderValue::from_static("1"));
    r
}

fn special_host(sh: &Shared, host: &str) -> Option<Response<RespBody>> {
    match host {
        // `curl https://wirecraft.check` through the proxy proves interception works end to end
        "wirecraft.check" => {
            let mut r = Response::new(full(r#"{"ok":true,"proxy":"WireCraft"}"#));
            r.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json"));
            Some(r)
        }
        "wirecraft.cert" | "wirecraft.ssl" => Some(cert_download(sh, "pem")),
        _ => None,
    }
}

fn cert_download(sh: &Shared, kind: &str) -> Response<RespBody> {
    let (body, ct, name): (Bytes, &str, &str) = match kind {
        "der" => (Bytes::copy_from_slice(sh.ca.der()), "application/x-x509-ca-cert", "wirecraft-root.der"),
        _ => (Bytes::from(sh.ca.pem().to_string()), "application/x-x509-ca-cert", "wirecraft-root.pem"),
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
        "<!doctype html><meta name=viewport content='width=device-width'><title>WireCraft</title>\
         <body style='font:16px system-ui;background:#0a0c0f;color:#e6e9ee;padding:32px;max-width:560px;margin:auto'>\
         <h2>WireCraft is running</h2><p>This device is talking to the WireCraft proxy at <code>{}</code>.</p>\
         <p><a style='color:#8b93ff' href='/cert.pem'>Download the WireCraft root certificate (.pem)</a><br>\
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
