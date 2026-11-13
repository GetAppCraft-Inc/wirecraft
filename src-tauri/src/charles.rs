//! Import of Charles Proxy JSON sessions (`.chlsj`, File → Export Session… → JSON Session File).
//!
//! The binary `.chls` format is an undocumented Java serialisation and is not supported; Charles can export
//! either JSON or HAR from it.

use base64::Engine;
use bytes::Bytes;
use serde_json::Value;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use wirecraft_core::store::{Headers, Timing, TlsInfo};
use wirecraft_core::{Flow, FlowState, FlowSummary, Store};

fn headers(v: &Value) -> Headers {
    v.pointer("/header/headers")
        .and_then(|h| h.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|h| Some((h.get("name")?.as_str()?.to_string(), h.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string())))
                .collect()
        })
        .unwrap_or_default()
}

fn header(h: &Headers, name: &str) -> Option<String> {
    h.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.clone())
}

/// Body bytes, and whether Charles already undid the Content-Encoding.
fn body(v: &Value) -> (Bytes, bool) {
    let b = &v["body"];
    let decoded = b["decoded"].as_bool().unwrap_or(true);
    if let Some(t) = b["text"].as_str() {
        return (Bytes::from(t.to_string()), decoded);
    }
    for key in ["encoded", "data", "base64"] {
        if let Some(e) = b[key].as_str() {
            return (base64::engine::general_purpose::STANDARD.decode(e.trim()).map(Bytes::from).unwrap_or_default(), decoded);
        }
    }
    (Bytes::new(), true)
}

fn time_ms(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => {
            OffsetDateTime::parse(s, &Rfc3339).ok().map(|t| t.unix_timestamp_nanos() as f64 / 1_000_000.0).or_else(|| {
                // Charles writes "+0000"-style offsets on some versions
                let fixed = if s.len() > 5 && (s.as_bytes()[s.len() - 5] == b'+' || s.as_bytes()[s.len() - 5] == b'-') {
                    format!("{}:{}", &s[..s.len() - 2], &s[s.len() - 2..])
                } else {
                    s.clone()
                };
                OffsetDateTime::parse(&fixed, &Rfc3339).ok().map(|t| t.unix_timestamp_nanos() as f64 / 1_000_000.0)
            })
        }
        _ => None,
    }
}

fn dur(v: &Value, k: &str) -> Option<f64> {
    v["durations"][k].as_f64().filter(|n| *n >= 0.0)
}

/// True when the text looks like a Charles JSON session (a top-level array of transactions).
pub fn looks_like(doc: &Value) -> bool {
    doc.as_array().map(|a| a.first().map(|e| e.get("host").is_some() && e.get("request").is_some()).unwrap_or(true)).unwrap_or(false)
}

pub fn import(store: &Store, doc: &Value, source: &str) -> Result<usize, String> {
    let entries = doc.as_array().ok_or("Not a Charles JSON session")?;
    let label = std::path::Path::new(source).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Charles".into());
    let mut n = 0;
    for e in entries {
        let Some(host) = e["host"].as_str() else { continue };
        if e["tunnel"].as_bool() == Some(true) && e["method"].as_str() == Some("CONNECT") && e["response"].is_null() {
            continue;
        }
        let scheme = e["scheme"].as_str().unwrap_or("http").to_ascii_lowercase();
        let port = e["actualPort"].as_u64().or_else(|| e["port"].as_u64()).unwrap_or(if scheme == "https" { 443 } else { 80 }) as u16;
        let mut path = e["path"].as_str().unwrap_or("/").to_string();
        if let Some(q) = e["query"].as_str().filter(|q| !q.is_empty()) {
            path = format!("{path}?{q}");
        }
        let method = e["method"].as_str().unwrap_or("GET").to_string();
        let req = &e["request"];
        let resp = &e["response"];
        let req_headers = headers(req);
        let mut resp_headers = headers(resp);
        let (req_body, _) = body(req);
        let (resp_body, decoded) = body(resp);
        if decoded {
            resp_headers.retain(|(k, _)| !k.eq_ignore_ascii_case("content-encoding"));
        }
        let status = resp["status"].as_u64().or_else(|| e["response"]["status"].as_u64()).filter(|s| *s > 0).map(|s| s as u16);
        let started = time_ms(&e["times"]["start"]).unwrap_or(0.0);
        let total = dur(e, "total").or_else(|| Some(time_ms(&e["times"]["end"])? - started).filter(|d| *d >= 0.0));
        let ct = header(&resp_headers, "content-type").or_else(|| resp["mimeType"].as_str().map(|s| s.to_string()));
        let req_ct = header(&req_headers, "content-type");
        let failed = e["status"].as_str().map(|s| !s.eq_ignore_ascii_case("COMPLETE")).unwrap_or(false);
        let error = if failed { e["errorMessage"].as_str().map(|s| s.to_string()).or_else(|| e["status"].as_str().map(|s| s.to_lowercase())) } else { None };
        let http_version = e["protocolVersion"].as_str().unwrap_or("HTTP/1.1").to_string();
        let tls = e.get("ssl").filter(|s| s.is_object()).map(|s| TlsInfo {
            version: s["protocol"].as_str().map(|p| p.replace("TLSv", "TLS ")),
            alpn: e.pointer("/alpn/protocol").and_then(|a| a.as_str()).map(|s| s.to_string()),
            sni: Some(host.to_string()),
            cipher: s["cipherSuite"].as_str().map(|s| s.to_string()),
        });
        let id = store.next_id();
        store.insert(Flow {
            summary: FlowSummary {
                id,
                method: method.clone(),
                scheme: scheme.clone(),
                host: host.to_string(),
                port,
                path: path.clone(),
                http_version: http_version.clone(),
                status,
                state: if error.is_some() || status.is_none() { FlowState::Error } else { FlowState::Complete },
                kind: wirecraft_core::body::classify(&method, &path, req_ct.as_deref(), ct.as_deref(), e["webSocket"].as_bool().unwrap_or(false))
                    .into(),
                content_type: ct,
                req_size: req["sizes"]["body"].as_u64().unwrap_or(req_body.len() as u64),
                resp_size: resp["sizes"]["body"].as_u64().unwrap_or(resp_body.len() as u64),
                started,
                duration_ms: total,
                client: Some(label.clone()),
                client_addr: "import".into(),
                tls: scheme == "https",
                xhr: header(&req_headers, "x-requested-with").is_some(),
                error,
                note: None,
                req_header_count: req_headers.len(),
                resp_header_count: resp_headers.len(),
                applied: vec![],
                ws_messages: 0,
            },
            req_headers,
            req_body,
            req_body_truncated: false,
            status_text: resp.pointer("/header/firstLine").and_then(|l| l.as_str()).and_then(|l| l.splitn(3, ' ').nth(2)).map(|s| s.to_string()),
            resp_http_version: Some(http_version),
            resp_headers,
            resp_body,
            resp_body_truncated: false,
            resp_trailers: vec![],
            timing: Timing {
                started,
                request_ms: dur(e, "request"),
                waiting_ms: dur(e, "latency"),
                download_ms: dur(e, "response"),
                total_ms: total,
            },
            tls,
            ws: vec![],
        });
        n += 1;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_a_transaction() {
        let doc: Value = serde_json::from_str(
            r#"[{
              "status": "COMPLETE", "method": "POST", "protocolVersion": "HTTP/2.0", "scheme": "https",
              "host": "api.example.com", "actualPort": 443, "path": "/v1/login", "query": "x=1", "tunnel": false,
              "times": { "start": "2026-11-13T09:00:00.000+00:00", "end": "2026-11-13T09:00:00.250+00:00" },
              "durations": { "total": 250, "request": 2, "latency": 200, "response": 48 },
              "ssl": { "protocol": "TLSv1.3", "cipherSuite": "TLS_AES_128_GCM_SHA256" },
              "request": { "header": { "firstLine": "POST /v1/login HTTP/2.0",
                  "headers": [{ "name": "content-type", "value": "application/json" }] },
                  "body": { "text": "{\"u\":\"a\"}", "charset": "UTF-8" } },
              "response": { "status": 401, "mimeType": "application/json",
                  "header": { "firstLine": "HTTP/2.0 401 Unauthorized",
                      "headers": [{ "name": "content-type", "value": "application/json" }, { "name": "content-encoding", "value": "gzip" }] },
                  "body": { "encoded": "e30=", "decoded": true } }
            }]"#,
        )
        .unwrap();
        assert!(looks_like(&doc));
        let store = Store::new(10);
        assert_eq!(import(&store, &doc, "/tmp/s.chlsj").unwrap(), 1);
        let f = store.get(store.ids()[0]).unwrap();
        assert_eq!(f.summary.path, "/v1/login?x=1");
        assert_eq!(f.summary.status, Some(401));
        assert_eq!(f.summary.duration_ms, Some(250.0));
        assert_eq!(f.status_text.as_deref(), Some("Unauthorized"));
        assert_eq!(&f.resp_body[..], b"{}");
        assert!(f.resp_headers.iter().all(|(k, _)| k != "content-encoding"));
        assert_eq!(f.tls.unwrap().version.as_deref(), Some("TLS 1.3"));
        assert!(!looks_like(&serde_json::json!({ "log": {} })));
    }
}
