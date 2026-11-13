//! WebSocket through the proxy: upgrade, frames in both directions, message capture, extension stripping.

use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use wirecraft_core::{CertAuthority, FlowState, Proxy, ProxyOptions, Store};

async fn read_head(s: &mut TcpStream) -> String {
    let mut buf = Vec::new();
    let mut b = [0u8; 1];
    while !buf.ends_with(b"\r\n\r\n") {
        s.read_exact(&mut b).await.unwrap();
        buf.push(b[0]);
    }
    String::from_utf8(buf).unwrap()
}

/// Read one frame (no fragmentation, payload < 126); returns (opcode, unmasked payload).
async fn read_frame(s: &mut TcpStream) -> (u8, Vec<u8>) {
    let mut h = [0u8; 2];
    s.read_exact(&mut h).await.unwrap();
    let len = (h[1] & 0x7f) as usize;
    let mask = if h[1] & 0x80 != 0 {
        let mut k = [0u8; 4];
        s.read_exact(&mut k).await.unwrap();
        Some(k)
    } else {
        None
    };
    let mut p = vec![0u8; len];
    s.read_exact(&mut p).await.unwrap();
    if let Some(k) = mask {
        for (i, b) in p.iter_mut().enumerate() {
            *b ^= k[i % 4];
        }
    }
    (h[0] & 0x0f, p)
}

fn frame(op: u8, payload: &[u8], mask: Option<[u8; 4]>) -> Vec<u8> {
    let mut v = vec![0x80 | op, (if mask.is_some() { 0x80 } else { 0 }) | payload.len() as u8];
    let mut p = payload.to_vec();
    if let Some(k) = mask {
        v.extend_from_slice(&k);
        for (i, b) in p.iter_mut().enumerate() {
            *b ^= k[i % 4];
        }
    }
    v.extend(p);
    v
}

#[tokio::test]
async fn websocket_messages_are_captured() {
    // echo server
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let up = l.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut s, _) = l.accept().await.unwrap();
        let head = read_head(&mut s).await.to_ascii_lowercase();
        let ext = head.contains("sec-websocket-extensions");
        s.write_all(b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: x\r\n\r\n").await.unwrap();
        loop {
            let (op, p) = read_frame(&mut s).await;
            if op == 8 {
                s.write_all(&frame(8, &[0x03, 0xe8], None)).await.unwrap();
                break;
            }
            let reply = format!("echo: {} ext={ext}", String::from_utf8_lossy(&p));
            s.write_all(&frame(1, reply.as_bytes(), None)).await.unwrap();
        }
    });

    let dir = std::env::temp_dir().join(format!("wirecraft-ws-test-{}", std::process::id()));
    let ca = Arc::new(CertAuthority::generate(&dir).unwrap());
    let store = Store::new(100);
    let proxy = Proxy::start(ca, store.clone(), ProxyOptions { listen: "127.0.0.1:0".parse().unwrap(), ..Default::default() }).await.unwrap();

    let mut c = TcpStream::connect(proxy.local_addr()).await.unwrap();
    c.write_all(
        format!(
            "GET http://{up}/socket HTTP/1.1\r\nHost: {up}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
             Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\
             Sec-WebSocket-Extensions: permessage-deflate\r\n\r\n"
        )
        .as_bytes(),
    )
    .await
    .unwrap();
    let head = read_head(&mut c).await;
    assert!(head.starts_with("HTTP/1.1 101"), "{head}");

    for msg in ["hello", "world"] {
        c.write_all(&frame(1, msg.as_bytes(), Some([7, 1, 9, 3]))).await.unwrap();
        let (op, p) = read_frame(&mut c).await;
        assert_eq!(op, 1);
        assert_eq!(String::from_utf8(p).unwrap(), format!("echo: {msg} ext=false"));
    }
    c.write_all(&frame(8, &[0x03, 0xe8], Some([1, 2, 3, 4]))).await.unwrap();
    let (op, _) = read_frame(&mut c).await;
    assert_eq!(op, 8);
    drop(c);

    // the flow records every message in order and finishes
    let mut flow = None;
    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        let f = store.get(store.ids()[0]).unwrap();
        if f.summary.state == FlowState::Complete {
            flow = Some(f);
            break;
        }
    }
    let f = flow.expect("flow completes");
    assert_eq!(f.summary.status, Some(101));
    assert_eq!(f.summary.kind, "WS");
    let got: Vec<(bool, &str, &str)> = f.ws.iter().map(|m| (m.outgoing, m.kind, m.data.as_str())).collect();
    assert_eq!(
        got,
        vec![
            (true, "text", "hello"),
            (false, "text", "echo: hello ext=false"),
            (true, "text", "world"),
            (false, "text", "echo: world ext=false"),
            (true, "close", "1000"),
            (false, "close", "1000"),
        ]
    );
    assert_eq!(f.summary.ws_messages, 6);
    assert!(f.summary.error.is_none(), "{:?}", f.summary.error);
}
