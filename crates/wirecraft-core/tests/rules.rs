//! End-to-end rule tests: a local upstream server, the real proxy, and raw HTTP/1.1 requests through it.

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
use xwire_core::rules::*;
use xwire_core::{CertAuthority, FlowState, Proxy, ProxyOptions, Store};

/// Upstream that echoes the method, path, `x-*` request headers and body.
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
                    r.headers_mut().insert("x-upstream", "1".parse().unwrap());
                    r.headers_mut().insert("content-type", "text/plain".parse().unwrap());
                    Ok::<_, Infallible>(r)
                });
                let _ = hyper::server::conn::http1::Builder::new().serve_connection(TokioIo::new(s), svc).await;
            });
        }
    });
    addr
}

struct Rig {
    proxy: Proxy,
    rules: SharedRules,
    store: Arc<Store>,
    up: SocketAddr,
}

async fn rig(r: Rules) -> Rig {
    let dir = std::env::temp_dir().join(format!("xwire-test-{}-{}", std::process::id(), rand_suffix()));
    let ca = Arc::new(CertAuthority::generate(&dir).unwrap());
    let store = Store::new(1000);
    let rules: SharedRules = Arc::new(parking_lot::RwLock::new(Arc::new(r)));
    let opts = ProxyOptions { listen: "127.0.0.1:0".parse().unwrap(), rules: rules.clone(), ..Default::default() };
    let proxy = Proxy::start(ca, store.clone(), opts).await.unwrap();
    Rig { proxy, rules, store, up: upstream().await }
}

fn rand_suffix() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    N.fetch_add(1, Ordering::Relaxed)
}

/// Send a raw request through the proxy; returns (status, headers lower-cased, body).
async fn send(rig: &Rig, method: &str, path: &str, headers: &[(&str, &str)], body: &str) -> (u16, String, String) {
    let mut s = TcpStream::connect(rig.proxy.local_addr()).await.unwrap();
    let mut req = format!("{method} http://{}{path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n", rig.up, rig.up);
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str(&format!("Content-Length: {}\r\n\r\n{body}", body.len()));
    s.write_all(req.as_bytes()).await.unwrap();
    let mut out = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), s.read_to_end(&mut out)).await.unwrap().unwrap();
    let text = String::from_utf8_lossy(&out).into_owned();
    let (head, body) = text.split_once("\r\n\r\n").unwrap();
    let status = head[9..12].parse().unwrap();
    (status, head.to_ascii_lowercase(), body.to_string())
}

fn m(url: &str) -> Matcher {
    Matcher { url: url.into(), methods: vec![] }
}

#[tokio::test]
async fn passthrough_and_request_rewrites() {
    let r = Rules {
        rewrite_on: true,
        rewrite: vec![
            Rewrite { matcher: m("*/api/*"), target: RewriteTarget::RequestHeader, key: "x-added".into(), value: "yes".into(), ..Default::default() },
            Rewrite { matcher: m("*/api/*"), target: RewriteTarget::QueryParam, key: "v".into(), value: "2".into(), ..Default::default() },
            Rewrite {
                matcher: m("*/api/*"),
                target: RewriteTarget::RequestBody,
                action: RewriteAction::Replace,
                find: "cat".into(),
                value: "dog".into(),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let rig = rig(r).await;
    let (st, _, body) = send(&rig, "GET", "/plain", &[], "").await;
    assert_eq!(st, 200);
    assert_eq!(body, "GET /plain [] ");
    let (st, _, body) = send(&rig, "POST", "/api/x?v=1", &[], "my cat").await;
    assert_eq!(st, 200);
    assert_eq!(body, "POST /api/x?v=2 [x-added=yes] my dog");
}

#[tokio::test]
async fn response_rewrites() {
    let r = Rules {
        rewrite_on: true,
        rewrite: vec![
            Rewrite { matcher: m(""), target: RewriteTarget::Status, value: "418".into(), ..Default::default() },
            Rewrite { matcher: m(""), target: RewriteTarget::ResponseHeader, action: RewriteAction::Remove, key: "x-upstream".into(), ..Default::default() },
            Rewrite {
                matcher: m(""),
                target: RewriteTarget::ResponseBody,
                action: RewriteAction::Replace,
                find: r"GET (\S+)".into(),
                value: "got $1".into(),
                regex: true,
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let rig = rig(r).await;
    let (st, head, body) = send(&rig, "GET", "/a", &[], "").await;
    assert_eq!(st, 418);
    assert!(!head.contains("x-upstream"));
    assert_eq!(body, "got /a [] ");
    tokio::time::sleep(Duration::from_millis(100)).await;
    let f = rig.store.summaries().pop().unwrap();
    assert_eq!(f.status, Some(418));
    assert!(f.applied.contains(&"Rewrite".to_string()));
}

#[tokio::test]
async fn map_local_map_remote_block() {
    let r = Rules {
        map_local_on: true,
        map_local: vec![MapLocal {
            matcher: m("*/local"),
            status: 201,
            headers: vec![("content-type".into(), "application/json".into())],
            body: r#"{"mocked":true}"#.into(),
            ..Default::default()
        }],
        block: vec![Block { matcher: m("*/blocked*"), ..Default::default() }],
        ..Default::default()
    };
    let rig = rig(r).await;
    let (st, head, body) = send(&rig, "GET", "/local", &[], "").await;
    assert_eq!((st, body.as_str()), (201, r#"{"mocked":true}"#));
    assert!(head.contains("application/json"));
    let (st, _, _) = send(&rig, "GET", "/blocked/x", &[], "").await;
    assert_eq!(st, 403);

    // map remote: everything under /old goes to the same upstream with a new prefix
    let mut r2 = (*rig.rules.read().clone()).clone();
    r2.map_remote_on = true;
    r2.map_remote = vec![MapRemote { matcher: m("*/old/*"), to: format!("http://{}/new", rig.up), ..Default::default() }];
    *rig.rules.write() = Arc::new(r2);
    let (st, _, body) = send(&rig, "GET", "/old/thing?q=1", &[], "").await;
    assert_eq!(st, 200);
    assert_eq!(body, "GET /new/old/thing?q=1 [] ");
}

#[tokio::test]
async fn breakpoints_edit_request_and_response() {
    let r = Rules {
        breakpoints_on: true,
        breakpoints: vec![Breakpoint { matcher: m("*/bp"), request: true, response: true, ..Default::default() }],
        ..Default::default()
    };
    let rig = rig(r).await;
    let breaks = rig.proxy.breaks().clone();
    let store = rig.store.clone();
    let driver = tokio::spawn(async move {
        // request phase
        let p = loop {
            if let Some(p) = breaks.list().pop() {
                break p;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        };
        assert_eq!(p.phase, "request");
        assert_eq!(store.get(p.flow_id).unwrap().summary.state, FlowState::Paused);
        assert!(breaks.resume(p.flow_id, Resume { body: Some("edited".into()), ..Default::default() }));
        // response phase
        let p = loop {
            if let Some(p) = breaks.list().pop() {
                break p;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        };
        assert_eq!(p.phase, "response");
        assert!(p.body.contains("edited"));
        assert!(breaks.resume(p.flow_id, Resume { status: Some(299), body: Some("final".into()), ..Default::default() }));
    });
    let (st, _, body) = send(&rig, "POST", "/bp", &[], "original").await;
    driver.await.unwrap();
    assert_eq!((st, body.as_str()), (299, "final"));
}

#[tokio::test]
async fn compose_sends_through_pipeline() {
    let rig = rig(Rules::default()).await;
    let url = format!("http://{}/composed", rig.up);
    let id = rig.proxy.send("PUT", &url, &vec![("x-hello".into(), "1".into())], Bytes::from("hi"), "test").unwrap();
    for _ in 0..100 {
        if rig.store.get(id).map(|f| f.summary.state == FlowState::Complete).unwrap_or(false) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let f = rig.store.get(id).unwrap();
    assert_eq!(f.summary.status, Some(200));
    assert_eq!(String::from_utf8_lossy(&f.resp_body), "PUT /composed [x-hello=1] hi");
}

#[tokio::test]
async fn throttle_slows_download() {
    let r = Rules {
        throttle: Throttle { enabled: true, latency_ms: 200, download_kbps: 0, upload_kbps: 0, ..Default::default() },
        ..Default::default()
    };
    let rig = rig(r).await;
    let t = std::time::Instant::now();
    let (st, _, _) = send(&rig, "GET", "/slow", &[], "").await;
    assert_eq!(st, 200);
    assert!(t.elapsed() >= Duration::from_millis(200));
}
