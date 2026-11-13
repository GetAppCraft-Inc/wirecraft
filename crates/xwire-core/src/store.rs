//! Captured flows and the change feed the UI subscribes to.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use bytes::Bytes;
use parking_lot::RwLock;
use serde::Serialize;
use tokio::sync::broadcast;

pub type Headers = Vec<(String, String)>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FlowState {
    /// Request sent, waiting for the response (or the response body is still streaming).
    Pending,
    Complete,
    Error,
    /// CONNECT tunnel passed through without decryption.
    Tunnel,
    /// Upgraded connection (WebSocket) that is still open.
    Live,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Timing {
    /// Unix epoch, milliseconds.
    pub started: f64,
    /// Time to receive the full request body from the client.
    pub request_ms: Option<f64>,
    /// Upstream wait: request sent → response headers.
    pub waiting_ms: Option<f64>,
    /// Response headers → last body byte.
    pub download_ms: Option<f64>,
    pub total_ms: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TlsInfo {
    pub version: Option<String>,
    pub alpn: Option<String>,
    pub sni: Option<String>,
    pub cipher: Option<String>,
}

/// Everything the table needs for one row. Sent to the UI on every change.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowSummary {
    pub id: u64,
    pub method: String,
    pub scheme: String,
    pub host: String,
    pub port: u16,
    /// Path plus query string.
    pub path: String,
    pub http_version: String,
    pub status: Option<u16>,
    pub state: FlowState,
    pub kind: String,
    pub content_type: Option<String>,
    pub req_size: u64,
    pub resp_size: u64,
    pub started: f64,
    pub duration_ms: Option<f64>,
    pub client: Option<String>,
    pub client_addr: String,
    pub tls: bool,
    pub xhr: bool,
    pub error: Option<String>,
    /// WebSocket frames or tunnel byte count, for live connections.
    pub note: Option<String>,
    pub req_header_count: usize,
    pub resp_header_count: usize,
}

#[derive(Debug, Clone)]
pub struct Flow {
    pub summary: FlowSummary,
    pub req_headers: Headers,
    pub req_body: Bytes,
    pub req_body_truncated: bool,
    pub status_text: Option<String>,
    pub resp_http_version: Option<String>,
    pub resp_headers: Headers,
    pub resp_body: Bytes,
    pub resp_body_truncated: bool,
    pub timing: Timing,
    pub tls: Option<TlsInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum StoreEvent {
    Upsert { flow: Box<FlowSummary> },
    Removed { ids: Vec<u64> },
    Cleared,
}

pub struct Store {
    flows: RwLock<Inner>,
    next_id: AtomicU64,
    recording: AtomicBool,
    tx: broadcast::Sender<StoreEvent>,
    max_flows: usize,
}

#[derive(Default)]
struct Inner {
    order: VecDeque<u64>,
    map: HashMap<u64, Flow>,
    bytes: u64,
}

impl Store {
    pub fn new(max_flows: usize) -> Arc<Self> {
        let (tx, _) = broadcast::channel(8192);
        Arc::new(Self {
            flows: RwLock::new(Inner::default()),
            next_id: AtomicU64::new(1),
            recording: AtomicBool::new(true),
            tx,
            max_flows,
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<StoreEvent> {
        self.tx.subscribe()
    }

    pub fn recording(&self) -> bool {
        self.recording.load(Ordering::Relaxed)
    }

    pub fn set_recording(&self, on: bool) {
        self.recording.store(on, Ordering::Relaxed);
    }

    pub fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    pub fn insert(&self, flow: Flow) {
        let summary = flow.summary.clone();
        let mut removed = Vec::new();
        {
            let mut g = self.flows.write();
            g.bytes += flow.summary.req_size + flow.summary.resp_size;
            g.order.push_back(flow.summary.id);
            g.map.insert(flow.summary.id, flow);
            while g.order.len() > self.max_flows {
                if let Some(old) = g.order.pop_front() {
                    if let Some(f) = g.map.remove(&old) {
                        g.bytes = g.bytes.saturating_sub(f.summary.req_size + f.summary.resp_size);
                    }
                    removed.push(old);
                }
            }
        }
        if !removed.is_empty() {
            let _ = self.tx.send(StoreEvent::Removed { ids: removed });
        }
        let _ = self.tx.send(StoreEvent::Upsert { flow: Box::new(summary) });
    }

    /// Mutate a flow in place and broadcast its new summary. No-op if the flow was cleared meanwhile.
    pub fn update(&self, id: u64, f: impl FnOnce(&mut Flow)) {
        let summary = {
            let mut g = self.flows.write();
            let inner = &mut *g;
            let Some(flow) = inner.map.get_mut(&id) else { return };
            let before = flow.summary.req_size + flow.summary.resp_size;
            f(flow);
            let after = flow.summary.req_size + flow.summary.resp_size;
            inner.bytes = inner.bytes.saturating_sub(before) + after;
            flow.summary.clone()
        };
        let _ = self.tx.send(StoreEvent::Upsert { flow: Box::new(summary) });
    }

    pub fn get(&self, id: u64) -> Option<Flow> {
        self.flows.read().map.get(&id).cloned()
    }

    pub fn with<R>(&self, id: u64, f: impl FnOnce(&Flow) -> R) -> Option<R> {
        self.flows.read().map.get(&id).map(f)
    }

    pub fn summaries(&self) -> Vec<FlowSummary> {
        let g = self.flows.read();
        g.order.iter().filter_map(|id| g.map.get(id)).map(|f| f.summary.clone()).collect()
    }

    pub fn ids(&self) -> Vec<u64> {
        self.flows.read().order.iter().copied().collect()
    }

    pub fn stats(&self) -> (usize, u64) {
        let g = self.flows.read();
        (g.order.len(), g.bytes)
    }

    pub fn clear(&self) {
        {
            let mut g = self.flows.write();
            *g = Inner::default();
        }
        let _ = self.tx.send(StoreEvent::Cleared);
    }

    pub fn remove(&self, ids: &[u64]) {
        {
            let mut g = self.flows.write();
            for id in ids {
                if let Some(f) = g.map.remove(id) {
                    g.bytes = g.bytes.saturating_sub(f.summary.req_size + f.summary.resp_size);
                }
            }
            let set: std::collections::HashSet<_> = ids.iter().collect();
            g.order.retain(|i| !set.contains(i));
        }
        let _ = self.tx.send(StoreEvent::Removed { ids: ids.to_vec() });
    }
}
