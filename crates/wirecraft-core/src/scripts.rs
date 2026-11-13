//! Scripts: user JavaScript that can rewrite requests and responses, or answer a request itself.
//!
//! Each script is a plain JS file that defines either or both of
//!
//! ```js
//! function onRequest(context, request) { request.headers["x-debug"] = "1"; return request; }
//! function onResponse(context, request, response) { response.status = 200; return response; }
//! ```
//!
//! `onRequest` may also return `{ response: { status, headers, body } }` to answer without contacting the server.
//! Functions may be `async`. `console.log` output goes to the script console (see [`logs`]).
//!
//! Sandbox: every run gets a fresh QuickJS runtime with no filesystem, network, timers or host bindings, a
//! memory cap and a wall-clock deadline (enforced through the interrupt handler). The only state that survives
//! between runs is `context.state`, a JSON object kept per script.

use std::collections::{HashMap, VecDeque};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine;
use parking_lot::Mutex;
use rquickjs::{Context, Runtime};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::rules::Matcher;
use crate::store::Headers;

const MEMORY_LIMIT: usize = 64 << 20;
const STACK_LIMIT: usize = 1 << 20;
const MAX_LOGS: usize = 1000;
const MAX_LOG_LEN: usize = 8 << 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Script {
    pub id: String,
    pub enabled: bool,
    pub name: String,
    #[serde(rename = "match")]
    pub matcher: Matcher,
    /// Run `onRequest`.
    pub request: bool,
    /// Run `onResponse` (the response is buffered, up to 32 MB).
    pub response: bool,
    pub code: String,
    /// Wall-clock budget per run; 0 = the default (1 s).
    pub timeout_ms: u64,
}

pub const TEMPLATE: &str = r#"// Runs for every request that matches this script.
// `request`  — { method, url, scheme, host, port, path, query, headers, body }
// `response` — { status, headers, body }
// `context`  — { flowId, script, client, state }  (state persists between runs)
// body is a string (decoded text) or null for binary bodies; headers are lower-case.

function onRequest(context, request) {
  // request.headers["x-wirecraft"] = "1";
  // return { response: { status: 200, headers: { "content-type": "application/json" }, body: { ok: true } } };
  return request;
}

function onResponse(context, request, response) {
  // if (response.headers["content-type"]?.includes("json")) {
  //   const data = JSON.parse(response.body);
  //   data.patched = true;
  //   response.body = data; // objects are serialised as JSON
  // }
  return response;
}
"#;

impl Default for Script {
    fn default() -> Self {
        Self {
            id: crate::rules::new_id(),
            enabled: true,
            name: String::new(),
            matcher: Matcher::default(),
            request: true,
            response: true,
            code: TEMPLATE.to_string(),
            timeout_ms: 0,
        }
    }
}

impl Script {
    fn timeout(&self) -> Duration {
        Duration::from_millis(if self.timeout_ms == 0 { 1000 } else { self.timeout_ms.clamp(10, 30_000) })
    }

    pub fn label(&self) -> String {
        if self.name.trim().is_empty() { "Script".into() } else { format!("Script · {}", self.name.trim()) }
    }
}

/// A request as a script sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptRequest {
    pub method: String,
    pub url: String,
    pub headers: Headers,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScriptResponse {
    pub status: u16,
    pub headers: Headers,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RequestOutcome {
    /// Continue with this (possibly modified) request.
    Continue(ScriptRequest),
    /// Answer the client directly.
    Respond(ScriptResponse),
}

// ---------------------------------------------------------------- console

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub seq: u64,
    pub at: f64,
    pub flow_id: Option<u64>,
    pub script: String,
    /// log | info | warn | error | debug
    pub level: String,
    pub message: String,
}

struct Console {
    seq: u64,
    lines: VecDeque<LogLine>,
}

fn console() -> &'static Mutex<Console> {
    static C: OnceLock<Mutex<Console>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(Console { seq: 0, lines: VecDeque::new() }))
}

fn now_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0)
}

pub fn log(flow_id: Option<u64>, script: &str, level: &str, message: &str) {
    let mut c = console().lock();
    c.seq += 1;
    let mut message = message.to_string();
    if message.len() > MAX_LOG_LEN {
        let mut cut = MAX_LOG_LEN;
        while !message.is_char_boundary(cut) {
            cut -= 1;
        }
        message.truncate(cut);
        message.push('…');
    }
    let line = LogLine { seq: c.seq, at: now_ms(), flow_id, script: script.to_string(), level: level.to_string(), message };
    c.lines.push_back(line);
    while c.lines.len() > MAX_LOGS {
        c.lines.pop_front();
    }
}

/// Console lines with `seq > after`, oldest first.
pub fn logs(after: u64) -> Vec<LogLine> {
    console().lock().lines.iter().filter(|l| l.seq > after).cloned().collect()
}

pub fn clear_logs() {
    console().lock().lines.clear();
}

fn states() -> &'static Mutex<HashMap<String, Value>> {
    static S: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
    S.get_or_init(Default::default)
}

/// Forget `context.state` for a script (or all scripts).
pub fn reset_state(id: Option<&str>) {
    match id {
        Some(id) => {
            states().lock().remove(id);
        }
        None => states().lock().clear(),
    }
}

// ---------------------------------------------------------------- JSON bridging

fn headers_to_json(h: &Headers) -> Value {
    let mut m = Map::new();
    for (k, v) in h {
        if k.starts_with(':') {
            continue;
        }
        let k = k.to_ascii_lowercase();
        match m.get_mut(&k) {
            Some(Value::Array(a)) => a.push(Value::String(v.clone())),
            Some(prev) => {
                let p = prev.take();
                *prev = Value::Array(vec![p, Value::String(v.clone())]);
            }
            None => {
                m.insert(k, Value::String(v.clone()));
            }
        }
    }
    Value::Object(m)
}

fn headers_from_json(v: &Value) -> Option<Headers> {
    let m = v.as_object()?;
    let mut out = Vec::new();
    for (k, v) in m {
        let k = k.trim().to_ascii_lowercase();
        if k.is_empty() {
            continue;
        }
        match v {
            Value::Null => {}
            Value::Array(a) => out.extend(a.iter().map(|x| (k.clone(), scalar(x)))),
            other => out.push((k, scalar(other))),
        }
    }
    Some(out)
}

fn scalar(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn body_to_json(b: &[u8]) -> (Value, Value) {
    match std::str::from_utf8(b) {
        Ok(s) => (Value::String(s.to_string()), Value::Null),
        Err(_) => (Value::Null, Value::String(base64::engine::general_purpose::STANDARD.encode(b))),
    }
}

/// The body a script returned: a string, an object (JSON), `bodyBase64`, or unchanged.
fn body_from_json(obj: &Value, original: &[u8]) -> Vec<u8> {
    match obj.get("body") {
        Some(Value::String(s)) => s.clone().into_bytes(),
        Some(Value::Null) | None => match obj.get("bodyBase64").and_then(|b| b.as_str()) {
            Some(b) => base64::engine::general_purpose::STANDARD.decode(b.trim()).unwrap_or_else(|_| original.to_vec()),
            None => original.to_vec(),
        },
        Some(other) => other.to_string().into_bytes(),
    }
}

fn split_url(url: &str) -> Value {
    let Ok(uri) = url.parse::<http::Uri>() else {
        return json!({});
    };
    let scheme = uri.scheme_str().unwrap_or("http").to_string();
    let port = uri.port_u16().unwrap_or(if scheme == "https" { 443 } else { 80 });
    let mut query = Map::new();
    for p in uri.query().unwrap_or("").split('&').filter(|p| !p.is_empty()) {
        let (k, v) = p.split_once('=').unwrap_or((p, ""));
        query.insert(k.to_string(), Value::String(v.to_string()));
    }
    json!({ "scheme": scheme, "host": uri.host().unwrap_or(""), "port": port, "path": uri.path(), "query": query })
}

fn request_json(r: &ScriptRequest) -> Value {
    let mut v = split_url(&r.url);
    let (body, b64) = body_to_json(&r.body);
    let o = v.as_object_mut().expect("object");
    o.insert("method".into(), r.method.clone().into());
    o.insert("url".into(), r.url.clone().into());
    o.insert("headers".into(), headers_to_json(&r.headers));
    o.insert("body".into(), body);
    o.insert("bodyBase64".into(), b64);
    v
}

fn response_json(r: &ScriptResponse) -> Value {
    let (body, b64) = body_to_json(&r.body);
    json!({ "status": r.status, "headers": headers_to_json(&r.headers), "body": body, "bodyBase64": b64 })
}

/// Build the outgoing URL from what the script returned. `url` wins when edited; otherwise edits to
/// host / path / query are applied to the original.
fn url_from_json(v: &Value, input: &Value, original: &str) -> String {
    let url = v.get("url").and_then(|u| u.as_str()).unwrap_or(original);
    if url != original {
        return url.to_string();
    }
    let field_changed = |k: &str| v.get(k).is_some() && v.get(k) != input.get(k);
    if !["scheme", "host", "port", "path", "query"].iter().any(|k| field_changed(k)) {
        return original.to_string();
    }
    let get = |k: &str| v.get(k).or_else(|| input.get(k)).cloned().unwrap_or(Value::Null);
    let scheme = get("scheme").as_str().unwrap_or("http").to_string();
    let host = get("host").as_str().unwrap_or("").to_string();
    let port = get("port").as_u64().or_else(|| get("port").as_str().and_then(|p| p.parse().ok()));
    let path = get("path").as_str().unwrap_or("/").to_string();
    let default = if scheme == "https" { 443 } else { 80 };
    let auth = match port {
        Some(p) if p != default => format!("{host}:{p}"),
        _ => host,
    };
    let q: Vec<String> = get("query")
        .as_object()
        .map(|m| {
            m.iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| {
                    let v = scalar(v);
                    if v.is_empty() { k.clone() } else { format!("{k}={v}") }
                })
                .collect()
        })
        .unwrap_or_default();
    let path = if path.starts_with('/') { path } else { format!("/{path}") };
    if q.is_empty() { format!("{scheme}://{auth}{path}") } else { format!("{scheme}://{auth}{path}?{}", q.join("&")) }
}

// ---------------------------------------------------------------- engine

const PRELUDE: &str = r#"
(function () {
  const logs = [];
  const fmt = (args) => args.map((x) => {
    if (typeof x === "string") return x;
    if (x instanceof Error) return String(x);
    try { const s = JSON.stringify(x); return s === undefined ? String(x) : s; } catch (e) { return String(x); }
  }).join(" ");
  const c = {};
  for (const level of ["log", "info", "warn", "error", "debug"]) c[level] = (...a) => { logs.push([level, fmt(a)]); };
  globalThis.console = c;
  globalThis.__logs = logs;
})();
"#;

const RUNNER: &str = r#"
(function () {
  const errText = (e) => (e && e.stack) ? String(e) + "\n" + e.stack : String(e);
  const input = JSON.parse(__input);
  let fn = null;
  try { fn = eval(__fn); } catch (e) { fn = null; }
  if (typeof fn !== "function") { globalThis.__result = JSON.stringify({ skipped: true }); return; }
  const ctx = input.context;
  const target = __fn === "onRequest" ? input.request : input.response;
  Promise.resolve()
    .then(() => __fn === "onRequest" ? fn(ctx, input.request) : fn(ctx, input.request, input.response))
    .then((v) => { globalThis.__result = JSON.stringify({ value: v === undefined ? target : v, state: ctx.state }); },
          (e) => { globalThis.__error = errText(e); });
})();
"#;

/// (result JSON, error text, console lines) read back from a finished run.
type RawRun = (Option<String>, Option<String>, Vec<(String, String)>);

struct RunOutput {
    value: Option<Value>,
    state: Option<Value>,
}

/// Run `function_name` from the script with `input` (`{context, request, response?}`) in a fresh sandbox.
fn run(script: &Script, flow_id: Option<u64>, function_name: &str, input: Value) -> Result<RunOutput, String> {
    let rt = Runtime::new().map_err(|e| format!("script runtime: {e}"))?;
    rt.set_memory_limit(MEMORY_LIMIT);
    rt.set_max_stack_size(STACK_LIMIT);
    let deadline = Instant::now() + script.timeout();
    rt.set_interrupt_handler(Some(Box::new(move || Instant::now() > deadline)));
    let ctx = Context::full(&rt).map_err(|e| format!("script context: {e}"))?;
    let name = script.label();

    let result: Result<RawRun, String> = ctx.with(|ctx| {
        let g = ctx.globals();
        let set = |k: &str, v: String| g.set(k, v).map_err(|e| e.to_string());
        set("__input", input.to_string())?;
        set("__fn", function_name.to_string())?;
        let timed_out = || Instant::now() > deadline;
        let timeout_msg = || format!("timed out after {} ms", script.timeout().as_millis());
        ctx.eval::<(), _>(PRELUDE).map_err(|e| e.to_string())?;
        // the user's code runs as a top-level script so `const onRequest = …` is visible to the runner
        let mut opts = rquickjs::context::EvalOptions::default();
        opts.global = true;
        opts.strict = false;
        opts.filename = Some("script.js".into());
        if let Err(e) = ctx.eval_with_options::<(), _>(script.code.as_str(), opts) {
            return Err(if timed_out() { timeout_msg() } else { exception_text(&ctx, e) });
        }
        if let Err(e) = ctx.eval::<(), _>(RUNNER) {
            return Err(if timed_out() { timeout_msg() } else { exception_text(&ctx, e) });
        }
        // drive promise jobs (async handlers)
        while ctx.execute_pending_job() {
            if timed_out() {
                return Err(timeout_msg());
            }
        }
        let res: Option<String> = g.get("__result").ok();
        let err: Option<String> = g.get("__error").ok();
        let logs: Vec<Vec<String>> = ctx
            .eval::<Option<String>, _>("JSON.stringify(globalThis.__logs || [])")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let logs = logs.into_iter().filter_map(|l| Some((l.first()?.clone(), l.get(1)?.clone()))).collect();
        Ok((res, err, logs))
    });

    let (res, err, logs) = match result {
        Ok(r) => r,
        Err(e) => {
            log(flow_id, &name, "error", &e);
            return Err(e);
        }
    };
    for (level, msg) in &logs {
        log(flow_id, &name, level, msg);
    }
    if let Some(e) = err {
        log(flow_id, &name, "error", &e);
        return Err(e.lines().next().unwrap_or("script error").to_string());
    }
    let Some(res) = res else {
        let e = "script did not finish (a promise never resolved)".to_string();
        log(flow_id, &name, "error", &e);
        return Err(e);
    };
    let v: Value = serde_json::from_str(&res).map_err(|e| format!("script result: {e}"))?;
    if v.get("skipped").is_some() {
        return Ok(RunOutput { value: None, state: None });
    }
    Ok(RunOutput { value: v.get("value").cloned(), state: v.get("state").cloned() })
}

fn context_json(script: &Script, flow_id: Option<u64>, client: Option<&str>) -> Value {
    let state = states().lock().get(&script.id).cloned().unwrap_or_else(|| json!({}));
    json!({ "flowId": flow_id, "script": script.name, "client": client, "state": state })
}

fn save_state(script: &Script, state: Option<Value>) {
    if let Some(s) = state.filter(|s| s.is_object()) {
        states().lock().insert(script.id.clone(), s);
    }
}

/// Run a script's `onRequest`. `Ok(None)` when the script has no `onRequest`.
pub fn run_request(script: &Script, flow_id: Option<u64>, client: Option<&str>, req: &ScriptRequest) -> Result<Option<RequestOutcome>, String> {
    let input_req = request_json(req);
    let input = json!({ "context": context_json(script, flow_id, client), "request": input_req });
    let out = run(script, flow_id, "onRequest", input)?;
    save_state(script, out.state);
    let Some(v) = out.value else { return Ok(None) };
    if v.is_null() {
        return Ok(Some(RequestOutcome::Continue(req.clone())));
    }
    if !v.is_object() {
        return Err("onRequest must return the request (or { response })".into());
    }
    if let Some(r) = v.get("response").filter(|r| r.is_object()) {
        let status = r.get("status").and_then(|s| s.as_u64()).unwrap_or(200) as u16;
        let mut headers = r.get("headers").and_then(headers_from_json).unwrap_or_default();
        let body = body_from_json(r, b"");
        if r.get("body").map(|b| b.is_object() || b.is_array()).unwrap_or(false)
            && !headers.iter().any(|(k, _)| k == "content-type")
        {
            headers.push(("content-type".into(), "application/json".into()));
        }
        return Ok(Some(RequestOutcome::Respond(ScriptResponse { status, headers, body })));
    }
    let method = v.get("method").and_then(|m| m.as_str()).map(|m| m.trim().to_ascii_uppercase()).filter(|m| !m.is_empty());
    let pseudo: Headers = req.headers.iter().filter(|(k, _)| k.starts_with(':')).cloned().collect();
    let headers = match v.get("headers").and_then(headers_from_json) {
        Some(mut h) => {
            h.splice(0..0, pseudo);
            h
        }
        None => req.headers.clone(),
    };
    Ok(Some(RequestOutcome::Continue(ScriptRequest {
        method: method.unwrap_or_else(|| req.method.clone()),
        url: url_from_json(&v, &input_req, &req.url),
        headers,
        body: body_from_json(&v, &req.body),
    })))
}

/// Run a script's `onResponse`. `Ok(None)` when the script has no `onResponse`.
pub fn run_response(
    script: &Script,
    flow_id: Option<u64>,
    client: Option<&str>,
    req: &ScriptRequest,
    resp: &ScriptResponse,
) -> Result<Option<ScriptResponse>, String> {
    let input = json!({ "context": context_json(script, flow_id, client), "request": request_json(req), "response": response_json(resp) });
    let out = run(script, flow_id, "onResponse", input)?;
    save_state(script, out.state);
    let Some(v) = out.value else { return Ok(None) };
    if v.is_null() {
        return Ok(Some(resp.clone()));
    }
    if !v.is_object() {
        return Err("onResponse must return the response".into());
    }
    let status = v.get("status").and_then(|s| s.as_u64().or_else(|| s.as_str().and_then(|s| s.trim().parse().ok())));
    let status = status.filter(|s| (100..=999).contains(s)).map(|s| s as u16).unwrap_or(resp.status);
    let mut headers = v.get("headers").and_then(headers_from_json).unwrap_or_else(|| resp.headers.clone());
    let body = body_from_json(&v, &resp.body);
    if v.get("body").map(|b| b.is_object() || b.is_array()).unwrap_or(false) && !headers.iter().any(|(k, _)| k == "content-type") {
        headers.push(("content-type".into(), "application/json".into()));
    }
    Ok(Some(ScriptResponse { status, headers, body }))
}

/// Compile-check a script and report which handlers it defines.
pub fn check(code: &str) -> Result<Vec<String>, String> {
    let rt = Runtime::new().map_err(|e| e.to_string())?;
    rt.set_memory_limit(MEMORY_LIMIT);
    let deadline = Instant::now() + Duration::from_millis(500);
    rt.set_interrupt_handler(Some(Box::new(move || Instant::now() > deadline)));
    let ctx = Context::full(&rt).map_err(|e| e.to_string())?;
    ctx.with(|ctx| {
        ctx.eval::<(), _>(PRELUDE).map_err(|e| e.to_string())?;
        let mut opts = rquickjs::context::EvalOptions::default();
        opts.global = true;
        opts.strict = false;
        opts.filename = Some("script.js".into());
        if let Err(e) = ctx.eval_with_options::<(), _>(code, opts) {
            return Err(if Instant::now() > deadline { "top-level code timed out".into() } else { exception_text(&ctx, e) });
        }
        let r: String = ctx
            .eval(
                r#"["onRequest", "onResponse"].filter((n) => { try { return typeof eval(n) === "function"; } catch (e) { return false; } }).join(",")"#,
            )
            .map_err(|e| exception_text(&ctx, e))?;
        Ok(r.split(',').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect())
    })
}

/// "Name: message (line N)" for a thrown JS value.
fn exception_text(ctx: &rquickjs::Ctx<'_>, e: rquickjs::Error) -> String {
    if !matches!(e, rquickjs::Error::Exception) {
        return e.to_string();
    }
    let v = ctx.catch();
    if let Some(ex) = v.as_exception() {
        let name: String = ex.get("name").unwrap_or_else(|_| "Error".into());
        let msg = ex.message().unwrap_or_default();
        let line = ex.stack().and_then(|s| {
            s.lines().find_map(|l| l.split("script.js:").nth(1).map(|r| r.trim_end_matches(')').to_string()))
        });
        return match line {
            Some(l) => format!("{name}: {msg} (line {l})"),
            None => format!("{name}: {msg}"),
        };
    }
    v.as_string().and_then(|s| s.to_string().ok()).unwrap_or_else(|| "uncaught exception".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script(code: &str) -> Script {
        Script { code: code.into(), ..Default::default() }
    }

    fn req() -> ScriptRequest {
        ScriptRequest {
            method: "GET".into(),
            url: "https://api.example.com/v1/items?page=1".into(),
            headers: vec![("accept".into(), "*/*".into()), ("cookie".into(), "a=1".into()), ("cookie".into(), "b=2".into())],
            body: vec![],
        }
    }

    #[test]
    fn modifies_request() {
        let s = script(
            r#"function onRequest(ctx, r) {
                r.headers["x-test"] = "yes";
                delete r.headers.accept;
                r.query.page = "2";
                r.method = "post";
                r.body = { hello: "world" };
                console.log("seen", r.headers.cookie);
                return r;
            }"#,
        );
        let Some(RequestOutcome::Continue(r)) = run_request(&s, Some(7), None, &req()).unwrap() else { panic!() };
        assert_eq!(r.method, "POST");
        assert_eq!(r.url, "https://api.example.com/v1/items?page=2");
        assert!(r.headers.contains(&("x-test".into(), "yes".into())));
        assert!(!r.headers.iter().any(|(k, _)| k == "accept"));
        assert_eq!(r.headers.iter().filter(|(k, _)| k == "cookie").count(), 2);
        assert_eq!(r.body, br#"{"hello":"world"}"#);
        assert!(logs(0).iter().any(|l| l.flow_id == Some(7) && l.message == r#"seen ["a=1","b=2"]"#));
    }

    #[test]
    fn can_answer_locally_and_async() {
        let s = script(r#"async function onRequest(ctx, r) { await null; return { response: { status: 418, body: { tea: true } } }; }"#);
        let Some(RequestOutcome::Respond(r)) = run_request(&s, None, None, &req()).unwrap() else { panic!() };
        assert_eq!(r.status, 418);
        assert_eq!(r.body, br#"{"tea":true}"#);
        assert!(r.headers.contains(&("content-type".into(), "application/json".into())));
    }

    #[test]
    fn response_and_state() {
        let s = script(
            r#"function onResponse(ctx, req, res) {
                ctx.state.n = (ctx.state.n || 0) + 1;
                const d = JSON.parse(res.body); d.n = ctx.state.n; res.body = JSON.stringify(d); res.status = 201;
                return res;
            }"#,
        );
        let resp = ScriptResponse { status: 200, headers: vec![], body: br#"{"a":1}"#.to_vec() };
        let a = run_response(&s, None, None, &req(), &resp).unwrap().unwrap();
        let b = run_response(&s, None, None, &req(), &resp).unwrap().unwrap();
        assert_eq!(a.status, 201);
        assert_eq!(a.body, br#"{"a":1,"n":1}"#);
        assert_eq!(b.body, br#"{"a":1,"n":2}"#);
        // no onRequest defined
        assert!(run_request(&s, None, None, &req()).unwrap().is_none());
    }

    #[test]
    fn errors_and_timeouts() {
        let e = run_request(&script("function onRequest() { throw new Error('boom'); }"), None, None, &req()).unwrap_err();
        assert!(e.contains("boom"), "{e}");
        let e = run_request(&script("function onRequest() { while (true) {} }"), None, None, &req()).unwrap_err();
        assert!(e.contains("timed out"), "{e}");
        let e = run_request(&script("function onRequest( {"), None, None, &req()).unwrap_err();
        assert!(e.contains("SyntaxError"), "{e}");
        let e = run_request(&script("function onRequest() { const a = []; while (true) a.push('x'.repeat(1000)); }"), None, None, &req())
            .unwrap_err();
        assert!(!e.is_empty());
    }

    #[test]
    fn check_reports_handlers() {
        assert_eq!(check(TEMPLATE).unwrap(), vec!["onRequest", "onResponse"]);
        assert!(check("const onResponse = (c, q, r) => r;").unwrap() == vec!["onResponse"]);
        assert!(check("function (").is_err());
    }
}
