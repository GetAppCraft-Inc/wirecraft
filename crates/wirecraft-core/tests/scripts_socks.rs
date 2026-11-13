//! End-to-end: Scripts and the SOCKS5 listener through the real proxy.

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use wirecraft_core::rules::*;
use wirecraft_core::scripts::Script;
use wirecraft_core::{CertAuthority, Proxy, ProxyOptions, Store};

async fn upstream() -> SocketAddr {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (s, _) = l.accept().await.unwrap();
            tokio::spawn(async move {
                let svc = service_fn(|req: Request<hyper::body::Incoming>| async move {
                    let (p, b) = req.into_parts();
                    let body = b.collect().await.unwrap().to_bytes();
                    let mut xs: Vec<String> = p
                        .headers
                        .iter()
                        .filter(|(k, _)| k.as_str().starts_with("x-"))
                        .map(|(k, v)| format!("{k}={}", v.to_str().unwrap()))
                        .collect();
                    xs.sort();
                    let text = format!("{} {} [{}] {}", p.method, p.uri, xs.join(","), String::from_utf8_lossy(&body));
                    let mut r = Response::new(Full::new(Bytes::from(text)));
                    r.headers_mut().insert("content-type", "text/plain".parse().unwrap());
                    Ok::<_, Infallible>(r)
                });
                let _ = hyper::server::conn::http1::Builder::new().serve_connection(TokioIo::new(s), svc).await;
            });
        }
    });
    addr
}

async fn start(r: Rules) -> (Proxy, Arc<Store>, SocketAddr) {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("wirecraft-test-ss-{}-{n}", std::process::id()));
    let ca = Arc::new(CertAuthority::generate(&dir).unwrap());
    let store = Store::new(1000);
    let rules: SharedRules = Arc::new(parking_lot::RwLock::new(Arc::new(r)));
    let opts = ProxyOptions { listen: "127.0.0.1:0".parse().unwrap(), rules, ..Default::default() };
    let proxy = Proxy::start(ca, store.clone(), opts).await.unwrap();
    (proxy, store, upstream().await)
}

async fn read_all(s: &mut TcpStream) -> (u16, String, String) {
    let mut out = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), s.read_to_end(&mut out)).await.unwrap().unwrap();
    let text = String::from_utf8_lossy(&out).into_owned();
    let (head, body) = text.split_once("\r\n\r\n").unwrap();
    (head[9..12].parse().unwrap(), head.to_ascii_lowercase(), body.to_string())
}

async fn send(proxy: &Proxy, up: SocketAddr, method: &str, path: &str, body: &str) -> (u16, String, String) {
    let mut s = TcpStream::connect(proxy.local_addr()).await.unwrap();
    let req = format!(
        "{method} http://{up}{path} HTTP/1.1\r\nHost: {up}\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    s.write_all(req.as_bytes()).await.unwrap();
    read_all(&mut s).await
}

fn script(url: &str, code: &str) -> Script {
    Script { matcher: Matcher { url: url.into(), methods: vec![] }, code: code.into(), ..Default::default() }
}

#[tokio::test]
async fn scripts_edit_request_and_response() {
    let r = Rules {
        scripts_on: true,
        scripts: vec![script(
            "*/api/*",
            r#"
            function onRequest(ctx, req) { req.headers["x-script"] = "on"; req.query.v = "9"; req.body = req.body.toUpperCase(); return req; }
            function onResponse(ctx, req, res) { res.status = 202; res.headers["x-out"] = "1"; res.body = res.body + " +patched"; return res; }
            "#,
        )],
        ..Default::default()
    };
    let (proxy, store, up) = start(r).await;
    let (st, head, body) = send(&proxy, up, "POST", "/api/x?v=1", "hello").await;
    assert_eq!(st, 202);
    assert!(head.contains("x-out: 1"));
    assert_eq!(body, "POST /api/x?v=9 [x-script=on] HELLO +patched");
    // untouched path
    let (st, _, body) = send(&proxy, up, "GET", "/other", "").await;
    assert_eq!((st, body.as_str()), (200, "GET /other [] "));
    tokio::time::sleep(Duration::from_millis(50)).await;
    let f = store.get(store.ids()[0]).unwrap();
    assert!(f.summary.applied.iter().any(|a| a == "Script"), "{:?}", f.summary.applied);
    assert_eq!(&f.resp_body[..], b"POST /api/x?v=9 [x-script=on] HELLO +patched");
}

#[tokio::test]
async fn script_answers_and_errors_pass_through() {
    let r = Rules {
        scripts_on: true,
        scripts: vec![
            script("*/mock", r#"function onRequest() { return { response: { status: 201, body: { mocked: true } } }; }"#),
            script("*/broken", r#"function onRequest() { throw new Error("nope"); }"#),
        ],
        ..Default::default()
    };
    let (proxy, store, up) = start(r).await;
    let (st, head, body) = send(&proxy, up, "GET", "/mock", "").await;
    assert_eq!((st, body.as_str()), (201, r#"{"mocked":true}"#));
    assert!(head.contains("content-type: application/json"));
    let (st, _, body) = send(&proxy, up, "GET", "/broken", "").await;
    assert_eq!((st, body.as_str()), (200, "GET /broken [] "));
    tokio::time::sleep(Duration::from_millis(50)).await;
    let broken = store.ids().into_iter().filter_map(|id| store.get(id)).find(|f| f.summary.path == "/broken").unwrap();
    assert!(broken.summary.applied.iter().any(|a| a == "Script error"));
    assert!(broken.summary.note.as_deref().unwrap_or("").contains("nope"));
}

#[tokio::test]
async fn socks5_connect_plain_http() {
    let (proxy, store, up) = start(Rules::default()).await;
    let mut s = TcpStream::connect(proxy.local_addr()).await.unwrap();
    s.write_all(&[0x05, 0x01, 0x00]).await.unwrap();
    let mut b = [0u8; 2];
    s.read_exact(&mut b).await.unwrap();
    assert_eq!(b, [0x05, 0x00]);
    let host = up.ip().to_string();
    let mut req = vec![0x05, 0x01, 0x00, 0x03, host.len() as u8];
    req.extend_from_slice(host.as_bytes());
    req.extend_from_slice(&up.port().to_be_bytes());
    s.write_all(&req).await.unwrap();
    let mut rep = [0u8; 10];
    s.read_exact(&mut rep).await.unwrap();
    assert_eq!(&rep[..2], &[0x05, 0x00]);
    s.write_all(format!("GET /via-socks HTTP/1.1\r\nHost: {up}\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();
    let (st, _, body) = read_all(&mut s).await;
    assert_eq!((st, body.as_str()), (200, "GET /via-socks [] "));
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(store.ids().into_iter().filter_map(|id| store.get(id)).any(|f| f.summary.path == "/via-socks" && f.summary.port == up.port()));
}

#[tokio::test]
async fn socks5_rejects_auth_only_clients() {
    let (proxy, _, _) = start(Rules::default()).await;
    let mut s = TcpStream::connect(proxy.local_addr()).await.unwrap();
    s.write_all(&[0x05, 0x01, 0x02]).await.unwrap();
    let mut b = [0u8; 2];
    s.read_exact(&mut b).await.unwrap();
    assert_eq!(b, [0x05, 0xff]);
}
