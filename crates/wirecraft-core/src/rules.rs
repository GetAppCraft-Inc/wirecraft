//! Traffic rules applied by the proxy: Map Local, Map Remote, Rewrite, Breakpoints, Block and Throttle.
//!
//! Rules are plain data (serde) so the app can persist them; the proxy holds an `Arc<Rules>` snapshot and
//! swaps it atomically when they change.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::body::{Body, Frame, SizeHint};
use parking_lot::Mutex;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::store::Headers;

/// Which requests a rule applies to.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Matcher {
    /// Glob over the URL. Without `://` it matches `host[:port]/path?query`; `*` matches any run of characters.
    /// An empty pattern matches everything.
    pub url: String,
    /// Upper-case methods; empty matches any.
    pub methods: Vec<String>,
}

impl Matcher {
    pub fn matches(&self, method: &str, url: &str) -> bool {
        if !self.methods.is_empty() && !self.methods.iter().any(|m| m.eq_ignore_ascii_case(method)) {
            return false;
        }
        let pat = self.url.trim();
        if pat.is_empty() || pat == "*" {
            return true;
        }
        if pat.contains("://") {
            wildcard(pat, url)
        } else {
            let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
            // a bare host pattern also matches every path on it
            wildcard(pat, rest) || (!pat.contains('/') && wildcard(&format!("{pat}/*"), rest))
        }
    }
}

/// Case-insensitive `*` wildcard match in linear time.
pub fn wildcard(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let (mut i, mut j) = (0usize, 0usize);
    let (mut star, mut mark) = (None::<usize>, 0usize);
    while j < t.len() {
        if i < p.len() && p[i] == '*' {
            star = Some(i);
            i += 1;
            mark = j;
        } else if i < p.len() && p[i] == t[j] {
            i += 1;
            j += 1;
        } else if let Some(s) = star {
            i = s + 1;
            mark += 1;
            j = mark;
        } else {
            return false;
        }
    }
    while i < p.len() && p[i] == '*' {
        i += 1;
    }
    i == p.len()
}

pub(crate) fn new_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    format!("r{t:x}{}", N.fetch_add(1, Ordering::Relaxed))
}

/// Serve a canned response instead of contacting the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MapLocal {
    pub id: String,
    pub enabled: bool,
    pub name: String,
    #[serde(rename = "match")]
    pub matcher: Matcher,
    pub status: u16,
    pub headers: Headers,
    /// Inline body, used when `file` is empty.
    pub body: String,
    /// Path of a file to serve as the body.
    pub file: String,
}

impl Default for MapLocal {
    fn default() -> Self {
        Self {
            id: new_id(),
            enabled: true,
            name: String::new(),
            matcher: Matcher::default(),
            status: 200,
            headers: vec![("content-type".into(), "application/json".into())],
            body: String::new(),
            file: String::new(),
        }
    }
}

/// Send matching requests to another server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MapRemote {
    pub id: String,
    pub enabled: bool,
    pub name: String,
    #[serde(rename = "match")]
    pub matcher: Matcher,
    /// `scheme://host[:port][/path-prefix]`. The request path is appended (after the prefix) unless
    /// `replace_path` is set.
    pub to: String,
    pub replace_path: bool,
}

impl Default for MapRemote {
    fn default() -> Self {
        Self { id: new_id(), enabled: true, name: String::new(), matcher: Matcher::default(), to: String::new(), replace_path: false }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum RewriteTarget {
    #[default]
    RequestHeader,
    ResponseHeader,
    RequestBody,
    ResponseBody,
    /// The URL (scheme://host/path?query) of the request.
    Url,
    /// A query parameter of the request URL.
    QueryParam,
    /// The response status code.
    Status,
}

impl RewriteTarget {
    pub fn is_response(self) -> bool {
        matches!(self, Self::ResponseHeader | Self::ResponseBody | Self::Status)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum RewriteAction {
    /// Set (replace or add) the header / query param / status / whole body.
    #[default]
    Set,
    /// Add a header or query param even if one with the same name exists.
    Add,
    /// Remove a header or query param.
    Remove,
    /// Find and replace inside the value / body / URL.
    Replace,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Rewrite {
    pub id: String,
    pub enabled: bool,
    pub name: String,
    #[serde(rename = "match")]
    pub matcher: Matcher,
    pub target: RewriteTarget,
    pub action: RewriteAction,
    /// Header or query parameter name (ignored for bodies, URL and status).
    pub key: String,
    /// New value, or the replacement for `Replace` (`$1` refers to regex groups).
    pub value: String,
    /// Text or regex to find, for `Replace`.
    pub find: String,
    pub regex: bool,
}

impl Default for Rewrite {
    fn default() -> Self {
        Self {
            id: new_id(),
            enabled: true,
            name: String::new(),
            matcher: Matcher::default(),
            target: RewriteTarget::RequestHeader,
            action: RewriteAction::Set,
            key: String::new(),
            value: String::new(),
            find: String::new(),
            regex: false,
        }
    }
}

impl Rewrite {
    fn replace_in(&self, s: &str) -> String {
        if self.find.is_empty() {
            return s.to_string();
        }
        if self.regex {
            match Regex::new(&self.find) {
                Ok(re) => re.replace_all(s, self.value.as_str()).into_owned(),
                Err(_) => s.to_string(),
            }
        } else {
            s.replace(&self.find, &self.value)
        }
    }

    /// Apply to a header list.
    pub fn apply_headers(&self, h: &mut Headers) {
        let key = self.key.trim().to_ascii_lowercase();
        if key.is_empty() && self.action != RewriteAction::Replace {
            return;
        }
        match self.action {
            RewriteAction::Set => {
                h.retain(|(k, _)| !k.eq_ignore_ascii_case(&key));
                h.push((key, self.value.clone()));
            }
            RewriteAction::Add => h.push((key, self.value.clone())),
            RewriteAction::Remove => h.retain(|(k, _)| !k.eq_ignore_ascii_case(&key)),
            RewriteAction::Replace => {
                for (k, v) in h.iter_mut() {
                    if key.is_empty() || k.eq_ignore_ascii_case(&key) {
                        *v = self.replace_in(v);
                    }
                }
            }
        }
    }

    pub fn apply_body(&self, body: &[u8]) -> Vec<u8> {
        match self.action {
            RewriteAction::Set | RewriteAction::Add => self.value.clone().into_bytes(),
            RewriteAction::Remove => Vec::new(),
            RewriteAction::Replace => match std::str::from_utf8(body) {
                Ok(s) => self.replace_in(s).into_bytes(),
                Err(_) => body.to_vec(),
            },
        }
    }

    pub fn apply_url(&self, url: &str) -> String {
        match self.target {
            RewriteTarget::Url => match self.action {
                RewriteAction::Set => self.value.clone(),
                RewriteAction::Replace => self.replace_in(url),
                _ => url.to_string(),
            },
            RewriteTarget::QueryParam => {
                let (base, query) = url.split_once('?').unwrap_or((url, ""));
                let mut params: Vec<(String, String)> = query
                    .split('&')
                    .filter(|p| !p.is_empty())
                    .map(|p| match p.split_once('=') {
                        Some((k, v)) => (k.to_string(), v.to_string()),
                        None => (p.to_string(), String::new()),
                    })
                    .collect();
                let key = self.key.trim();
                match self.action {
                    RewriteAction::Set => {
                        params.retain(|(k, _)| k != key);
                        params.push((key.to_string(), self.value.clone()));
                    }
                    RewriteAction::Add => params.push((key.to_string(), self.value.clone())),
                    RewriteAction::Remove => params.retain(|(k, _)| k != key),
                    RewriteAction::Replace => {
                        for (k, v) in params.iter_mut() {
                            if key.is_empty() || k == key {
                                *v = self.replace_in(v);
                            }
                        }
                    }
                }
                if params.is_empty() {
                    base.to_string()
                } else {
                    let q: Vec<String> =
                        params.iter().map(|(k, v)| if v.is_empty() { k.clone() } else { format!("{k}={v}") }).collect();
                    format!("{base}?{}", q.join("&"))
                }
            }
            _ => url.to_string(),
        }
    }

    pub fn apply_status(&self, status: u16) -> u16 {
        match self.action {
            RewriteAction::Set | RewriteAction::Add => self.value.trim().parse().unwrap_or(status),
            RewriteAction::Replace => self.replace_in(&status.to_string()).trim().parse().unwrap_or(status),
            RewriteAction::Remove => status,
        }
    }
}

/// Pause matching exchanges so they can be edited before they continue.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Breakpoint {
    pub id: String,
    pub enabled: bool,
    pub name: String,
    #[serde(rename = "match")]
    pub matcher: Matcher,
    pub request: bool,
    pub response: bool,
}

impl Default for Breakpoint {
    fn default() -> Self {
        Self { id: new_id(), enabled: true, name: String::new(), matcher: Matcher::default(), request: true, response: false }
    }
}

/// Refuse matching requests (the client gets a 403 or a dropped connection).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Block {
    pub id: String,
    pub enabled: bool,
    #[serde(rename = "match")]
    pub matcher: Matcher,
}

impl Default for Block {
    fn default() -> Self {
        Self { id: new_id(), enabled: true, matcher: Matcher::default() }
    }
}

/// Simulate a slow network.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Throttle {
    pub enabled: bool,
    /// Name of the preset the values came from (display only).
    pub preset: String,
    /// Added before each request is sent upstream.
    pub latency_ms: u64,
    /// Download bandwidth in kilobits per second; 0 = unlimited.
    pub download_kbps: u64,
    /// Upload bandwidth in kilobits per second; 0 = unlimited.
    pub upload_kbps: u64,
    /// Host globs to throttle; empty = every host.
    pub hosts: Vec<String>,
}

impl Default for Throttle {
    fn default() -> Self {
        Self { enabled: false, preset: "3G".into(), latency_ms: 300, download_kbps: 1600, upload_kbps: 750, hosts: vec![] }
    }
}

impl Throttle {
    pub fn applies(&self, host: &str) -> bool {
        self.enabled && (self.hosts.is_empty() || self.hosts.iter().any(|h| crate::proxy::glob_match(h, host)))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Rules {
    pub map_local: Vec<MapLocal>,
    pub map_remote: Vec<MapRemote>,
    pub rewrite: Vec<Rewrite>,
    pub breakpoints: Vec<Breakpoint>,
    pub block: Vec<Block>,
    pub throttle: Throttle,
    pub scripts: Vec<crate::scripts::Script>,
    /// Master switches per tool, so a whole tool can be turned off without losing its rules.
    pub map_local_on: bool,
    pub map_remote_on: bool,
    pub rewrite_on: bool,
    pub breakpoints_on: bool,
    pub scripts_on: bool,
}

impl Rules {
    pub fn map_local_for(&self, method: &str, url: &str) -> Option<&MapLocal> {
        if !self.map_local_on {
            return None;
        }
        self.map_local.iter().find(|r| r.enabled && r.matcher.matches(method, url))
    }

    pub fn map_remote_for(&self, method: &str, url: &str) -> Option<&MapRemote> {
        if !self.map_remote_on {
            return None;
        }
        self.map_remote.iter().find(|r| r.enabled && !r.to.trim().is_empty() && r.matcher.matches(method, url))
    }

    pub fn rewrites_for<'a>(&'a self, method: &'a str, url: &'a str) -> impl Iterator<Item = &'a Rewrite> + 'a {
        self.rewrite.iter().filter(move |r| self.rewrite_on && r.enabled && r.matcher.matches(method, url))
    }

    pub fn breakpoint_for(&self, method: &str, url: &str, response: bool) -> Option<&Breakpoint> {
        if !self.breakpoints_on {
            return None;
        }
        self.breakpoints
            .iter()
            .find(|b| b.enabled && (if response { b.response } else { b.request }) && b.matcher.matches(method, url))
    }

    pub fn scripts_for<'a>(&'a self, method: &'a str, url: &'a str, response: bool) -> impl Iterator<Item = &'a crate::scripts::Script> + 'a {
        self.scripts.iter().filter(move |s| {
            self.scripts_on && s.enabled && (if response { s.response } else { s.request }) && s.matcher.matches(method, url)
        })
    }

    pub fn blocked(&self, method: &str, url: &str) -> bool {
        self.block.iter().any(|b| b.enabled && b.matcher.matches(method, url))
    }

    /// Short labels for the tools that are switched on and have enabled rules (for the status bar).
    pub fn active_counts(&self) -> HashMap<&'static str, usize> {
        let mut m = HashMap::new();
        m.insert("mapLocal", if self.map_local_on { self.map_local.iter().filter(|r| r.enabled).count() } else { 0 });
        m.insert("mapRemote", if self.map_remote_on { self.map_remote.iter().filter(|r| r.enabled).count() } else { 0 });
        m.insert("rewrite", if self.rewrite_on { self.rewrite.iter().filter(|r| r.enabled).count() } else { 0 });
        m.insert("scripts", if self.scripts_on { self.scripts.iter().filter(|r| r.enabled).count() } else { 0 });
        m.insert("breakpoints", if self.breakpoints_on { self.breakpoints.iter().filter(|r| r.enabled).count() } else { 0 });
        m
    }
}

/// Apply `to` from a Map Remote rule to a URL.
pub fn map_remote_url(rule: &MapRemote, url: &str) -> Option<String> {
    let to = rule.to.trim().trim_end_matches('/');
    let (to_scheme, to_rest) = to.split_once("://").unwrap_or(("https", to));
    let (to_auth, to_path) = match to_rest.find('/') {
        Some(i) => (&to_rest[..i], &to_rest[i..]),
        None => (to_rest, ""),
    };
    if to_auth.is_empty() {
        return None;
    }
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let path = rest.find('/').map(|i| &rest[i..]).unwrap_or("/");
    let new_path = if rule.replace_path {
        // keep the query string
        let q = path.find('?').map(|i| &path[i..]).unwrap_or("");
        format!("{}{q}", if to_path.is_empty() { "/" } else { to_path })
    } else {
        format!("{to_path}{path}")
    };
    Some(format!("{to_scheme}://{to_auth}{new_path}"))
}

// ---------------------------------------------------------------- breakpoints

/// What the user sends back when resuming a paused exchange.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Resume {
    /// Drop the exchange: the client gets an error response.
    pub abort: bool,
    // request phase
    pub method: Option<String>,
    pub url: Option<String>,
    // response phase
    pub status: Option<u16>,
    // both phases
    pub headers: Option<Headers>,
    /// UTF-8 body; None leaves the body unchanged.
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Paused {
    pub flow_id: u64,
    /// "request" or "response"
    pub phase: &'static str,
    pub method: String,
    pub url: String,
    pub status: Option<u16>,
    pub headers: Headers,
    pub body: String,
    /// Body is not valid UTF-8 and cannot be edited as text.
    pub binary: bool,
}

/// Exchanges currently held at a breakpoint.
#[derive(Default)]
pub struct Breaks {
    waiting: Mutex<Vec<(Paused, oneshot::Sender<Resume>)>>,
}

impl std::fmt::Debug for Breaks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Breaks").field("waiting", &self.waiting.lock().len()).finish()
    }
}

impl Breaks {
    pub fn pause(&self, p: Paused) -> oneshot::Receiver<Resume> {
        let (tx, rx) = oneshot::channel();
        let mut w = self.waiting.lock();
        w.retain(|(_, s)| !s.is_closed());
        w.push((p, tx));
        rx
    }

    pub fn list(&self) -> Vec<Paused> {
        let mut w = self.waiting.lock();
        w.retain(|(_, s)| !s.is_closed());
        w.iter().map(|(p, _)| p.clone()).collect()
    }

    /// Continue a paused exchange. Returns false when it is no longer waiting (client went away).
    pub fn resume(&self, flow_id: u64, r: Resume) -> bool {
        let mut w = self.waiting.lock();
        match w.iter().position(|(p, _)| p.flow_id == flow_id) {
            Some(i) => w.remove(i).1.send(r).is_ok(),
            None => false,
        }
    }

    pub fn resume_all(&self) {
        let mut w = self.waiting.lock();
        for (_, tx) in w.drain(..) {
            let _ = tx.send(Resume::default());
        }
    }
}

// ---------------------------------------------------------------- throttling

pin_project_lite::pin_project! {
    /// Delays body frames so the body arrives no faster than `bytes_per_sec`.
    pub struct Throttled<B> {
        #[pin]
        inner: B,
        bytes_per_sec: f64,
        sleep: Option<Pin<Box<tokio::time::Sleep>>>,
        held: Option<Frame<Bytes>>,
    }
}

impl<B> Throttled<B> {
    pub fn new(inner: B, kbps: u64) -> Self {
        Self { inner, bytes_per_sec: (kbps as f64 * 1000.0 / 8.0).max(1.0), sleep: None, held: None }
    }
}

impl<B> Body for Throttled<B>
where
    B: Body<Data = Bytes>,
{
    type Data = Bytes;
    type Error = B::Error;

    fn poll_frame(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Result<Frame<Bytes>, B::Error>>> {
        let mut this = self.project();
        if let Some(s) = this.sleep.as_mut() {
            if s.as_mut().poll(cx).is_pending() {
                return Poll::Pending;
            }
            *this.sleep = None;
            if let Some(f) = this.held.take() {
                return Poll::Ready(Some(Ok(f)));
            }
        }
        match this.inner.as_mut().poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                let n = frame.data_ref().map(|d| d.len()).unwrap_or(0);
                if n == 0 {
                    return Poll::Ready(Some(Ok(frame)));
                }
                let delay = Duration::from_secs_f64(n as f64 / *this.bytes_per_sec);
                let mut s = Box::pin(tokio::time::sleep(delay));
                if s.as_mut().poll(cx).is_ready() {
                    return Poll::Ready(Some(Ok(frame)));
                }
                *this.sleep = Some(s);
                *this.held = Some(frame);
                Poll::Pending
            }
            other => other,
        }
    }

    fn is_end_stream(&self) -> bool {
        self.held.is_none() && self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

use std::future::Future;

/// Upload delay for a request body of `len` bytes.
pub fn upload_delay(t: &Throttle, len: usize) -> Duration {
    let mut d = Duration::from_millis(t.latency_ms);
    if t.upload_kbps > 0 {
        d += Duration::from_secs_f64(len as f64 / (t.upload_kbps as f64 * 1000.0 / 8.0));
    }
    d
}

/// Collect a body with a size cap; returns None if it is larger than `cap`.
pub async fn collect_capped(
    mut body: http_body_util::combinators::UnsyncBoxBody<Bytes, Box<dyn std::error::Error + Send + Sync>>,
    cap: usize,
) -> Result<Option<Bytes>, String> {
    let mut out = bytes::BytesMut::new();
    while let Some(frame) = body.frame().await {
        let frame = frame.map_err(|e| e.to_string())?;
        if let Some(d) = frame.data_ref() {
            if out.len() + d.len() > cap {
                return Ok(None);
            }
            out.extend_from_slice(d);
        }
    }
    Ok(Some(out.freeze()))
}

pub type SharedRules = Arc<parking_lot::RwLock<Arc<Rules>>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard_matches() {
        assert!(wildcard("*.example.com/*", "api.example.com/v1/x"));
        assert!(wildcard("api.example.com/v1/*", "api.example.com/v1/users?a=1"));
        assert!(!wildcard("api.example.com/v2/*", "api.example.com/v1/users"));
        assert!(wildcard("*", ""));
        assert!(wildcard("a*b*c", "aXXbYYc"));
        assert!(!wildcard("a*b*c", "aXXbYY"));
    }

    #[test]
    fn matcher_without_scheme() {
        let m = Matcher { url: "api.example.com".into(), methods: vec![] };
        assert!(m.matches("GET", "https://api.example.com/v1/users"));
        assert!(!m.matches("GET", "https://example.com/"));
        let m = Matcher { url: "https://api.example.com/v1/*".into(), methods: vec!["POST".into()] };
        assert!(m.matches("post", "https://api.example.com/v1/x"));
        assert!(!m.matches("GET", "https://api.example.com/v1/x"));
    }

    #[test]
    fn remote_mapping() {
        let r = MapRemote { to: "http://localhost:3000".into(), ..Default::default() };
        assert_eq!(map_remote_url(&r, "https://api.example.com/v1/x?y=1").unwrap(), "http://localhost:3000/v1/x?y=1");
        let r = MapRemote { to: "https://staging.example.com/api".into(), replace_path: true, ..Default::default() };
        assert_eq!(map_remote_url(&r, "https://api.example.com/v1/x?y=1").unwrap(), "https://staging.example.com/api?y=1");
    }

    #[test]
    fn rewrites() {
        let r = Rewrite { target: RewriteTarget::QueryParam, action: RewriteAction::Set, key: "a".into(), value: "2".into(), ..Default::default() };
        assert_eq!(r.apply_url("https://x/p?a=1&b=3"), "https://x/p?b=3&a=2");
        let r = Rewrite { action: RewriteAction::Replace, find: r#""(\d+)""#.into(), value: "$1".into(), regex: true, ..Default::default() };
        assert_eq!(r.apply_body(br#"{"n":"42"}"#), br#"{"n":42}"#.to_vec());
        let mut h: Headers = vec![("x-a".into(), "1".into())];
        Rewrite { key: "X-A".into(), action: RewriteAction::Remove, ..Default::default() }.apply_headers(&mut h);
        assert!(h.is_empty());
    }
}
