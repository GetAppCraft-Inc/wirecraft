//! Run the proxy without the UI and print each finished flow: `cargo run --example headless -- 127.0.0.1:9090`
use std::sync::Arc;
use xwire_core::{CertAuthority, Proxy, ProxyOptions, Store, StoreEvent, FlowState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into())).init();
    let listen = std::env::args().nth(1).unwrap_or_else(|| "127.0.0.1:9090".into()).parse()?;
    let dir = std::env::var("XWIRE_CA_DIR").map(Into::into).unwrap_or_else(|_| xwire_core::ca::default_ca_dir());
    let ca = Arc::new(CertAuthority::load_or_create(&dir)?);
    println!("CA: {} ({})", ca.info().common_name, ca.info().cert_path);
    let store = Store::new(100_000);
    let mut rx = store.subscribe();
    let proxy = Proxy::start(ca, store.clone(), ProxyOptions { listen, ..Default::default() }).await?;
    println!("listening on {}", proxy.local_addr());
    loop {
        match rx.recv().await {
            Ok(StoreEvent::Upsert { flow }) if flow.state != FlowState::Pending && flow.state != FlowState::Live => {
                println!("#{:<4} {:7} {:>4} {}://{}{} {} {}B {:.0}ms {:?} {}",
                    flow.id, flow.method, flow.status.map(|s| s.to_string()).unwrap_or("-".into()),
                    flow.scheme, flow.host, flow.path, flow.kind, flow.resp_size,
                    flow.duration_ms.unwrap_or(0.0), flow.state, flow.client.clone().unwrap_or_default());
                if let Some(e) = &flow.error { println!("      error: {e}"); }
            }
            Ok(_) => {}
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
            Err(_) => break,
        }
    }
    Ok(())
}
