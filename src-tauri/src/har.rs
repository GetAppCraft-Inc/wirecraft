//! HAR 1.2 export.

use base64::Engine;
use serde_json::{Value, json};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use wirecraft_core::Flow;
use wirecraft_core::store::Headers;

fn header(h: &Headers, name: &str) -> Option<String> {
    h.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.clone())
}

fn headers_json(h: &Headers) -> Value {
    Value::Array(h.iter().filter(|(k, _)| !k.starts_with(':')).map(|(k, v)| json!({ "name": k, "value": v })).collect())
}

fn query_json(path: &str) -> Value {
    let Some((_, q)) = path.split_once('?') else { return json!([]) };
    Value::Array(
        q.split('&')
            .filter(|p| !p.is_empty())
            .map(|p| {
                let (k, v) = p.split_once('=').unwrap_or((p, ""));
                json!({ "name": k, "value": v })
            })
            .collect(),
    )
}

fn cookies_json(h: &Headers, response: bool) -> Value {
    let mut out = Vec::new();
    for (k, v) in h {
        if !response && k.eq_ignore_ascii_case("cookie") {
            for c in v.split(';') {
                if let Some((n, val)) = c.trim().split_once('=') {
                    out.push(json!({ "name": n, "value": val }));
                }
            }
        } else if response && k.eq_ignore_ascii_case("set-cookie") {
            let first = v.split(';').next().unwrap_or("");
            if let Some((n, val)) = first.trim().split_once('=') {
                out.push(json!({ "name": n, "value": val }));
            }
        }
    }
    Value::Array(out)
}

fn iso(ms: f64) -> String {
    OffsetDateTime::from_unix_timestamp_nanos((ms * 1_000_000.0) as i128)
        .ok()
        .and_then(|t| t.format(&Rfc3339).ok())
        .unwrap_or_default()
}

fn http_version(v: &str) -> String {
    match v {
        "HTTP/2" | "HTTP/2.0" | "h2" => "h2".into(),
        other => other.to_string(),
    }
}

fn entry(f: &Flow) -> Value {
    let s = &f.summary;
    let default_port = if s.scheme == "https" { 443 } else { 80 };
    let authority = if s.port == default_port { s.host.clone() } else { format!("{}:{}", s.host, s.port) };
    let url = format!("{}://{}{}", s.scheme, authority, s.path);

    let (req_body, _) = wirecraft_core::proxy::decoded_body(&f.req_body, &f.req_headers);
    let post = if req_body.is_empty() {
        Value::Null
    } else {
        json!({
            "mimeType": header(&f.req_headers, "content-type").unwrap_or_default(),
            "text": String::from_utf8_lossy(&req_body),
        })
    };

    let (resp_body, _) = wirecraft_core::proxy::decoded_body(&f.resp_body, &f.resp_headers);
    let mime = header(&f.resp_headers, "content-type").unwrap_or_default();
    let content = match std::str::from_utf8(&resp_body) {
        Ok(t) => json!({ "size": resp_body.len(), "mimeType": mime, "text": t }),
        Err(_) => json!({
            "size": resp_body.len(),
            "mimeType": mime,
            "text": base64::engine::general_purpose::STANDARD.encode(&resp_body),
            "encoding": "base64",
        }),
    };

    let t = &f.timing;
    let send = t.request_ms.unwrap_or(0.0);
    let wait = t.waiting_ms.unwrap_or(0.0);
    let receive = t.download_ms.unwrap_or(0.0);
    let total = t.total_ms.or(s.duration_ms).unwrap_or(send + wait + receive);

    let mut request = json!({
        "method": s.method,
        "url": url,
        "httpVersion": http_version(&s.http_version),
        "cookies": cookies_json(&f.req_headers, false),
        "headers": headers_json(&f.req_headers),
        "queryString": query_json(&s.path),
        "headersSize": -1,
        "bodySize": f.req_body.len(),
    });
    if !post.is_null() {
        request["postData"] = post;
    }

    let mut e = json!({
        "startedDateTime": iso(t.started),
        "time": total,
        "request": request,
        "response": {
            "status": s.status.unwrap_or(0),
            "statusText": f.status_text.clone().unwrap_or_default(),
            "httpVersion": http_version(f.resp_http_version.as_deref().unwrap_or(&s.http_version)),
            "cookies": cookies_json(&f.resp_headers, true),
            "headers": headers_json(&f.resp_headers),
            "content": content,
            "redirectURL": header(&f.resp_headers, "location").unwrap_or_default(),
            "headersSize": -1,
            "bodySize": s.resp_size,
        },
        "cache": {},
        "timings": { "blocked": -1, "dns": -1, "connect": -1, "ssl": -1, "send": send, "wait": wait, "receive": receive },
    });
    if let Some(err) = &s.error {
        e["_error"] = json!(err);
    }
    if let Some(c) = &s.client {
        e["_client"] = json!(c);
    }
    if !f.ws.is_empty() {
        e["_resourceType"] = json!("websocket");
        e["_webSocketMessages"] = Value::Array(
            f.ws.iter()
                .map(|m| json!({ "type": if m.outgoing { "send" } else { "receive" }, "time": m.at / 1000.0, "opcode": if m.kind == "text" { 1 } else { 2 }, "data": m.data }))
                .collect(),
        );
    }
    e
}

pub fn build(flows: &[Flow], version: &str) -> Value {
    json!({
        "log": {
            "version": "1.2",
            "creator": { "name": "WireCraft", "version": version },
            "pages": [],
            "entries": flows.iter().map(entry).collect::<Vec<_>>(),
        }
    })
}

// ---------------------------------------------------------------- import

fn headers_from(v: &Value) -> Headers {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|h| Some((h.get("name")?.as_str()?.to_string(), h.get("value")?.as_str().unwrap_or("").to_string())))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_time(s: &str) -> f64 {
    OffsetDateTime::parse(s, &Rfc3339).map(|t| t.unix_timestamp_nanos() as f64 / 1_000_000.0).unwrap_or(0.0)
}

fn ms(v: Option<&Value>) -> Option<f64> {
    v.and_then(|v| v.as_f64()).filter(|n| *n >= 0.0)
}

/// Load the entries of a HAR document into the store. Returns how many were imported.
pub fn import(store: &wirecraft_core::Store, text: &str, source: &str) -> Result<usize, String> {
    use bytes::Bytes;
    use wirecraft_core::store::{Timing, WsMessage};
    use wirecraft_core::{Flow, FlowState, FlowSummary};

    let doc: Value = serde_json::from_str(text).map_err(|e| format!("Not a HAR file: {e}"))?;
    let entries = doc.pointer("/log/entries").and_then(|e| e.as_array()).ok_or("Not a HAR file: no log.entries")?;
    let label = std::path::Path::new(source).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "HAR".into());
    let mut n = 0;
    for e in entries {
        let req = &e["request"];
        let resp = &e["response"];
        let Some(url) = req["url"].as_str() else { continue };
        let Ok(uri) = url.parse::<http::Uri>() else { continue };
        let scheme = uri.scheme_str().unwrap_or("http").to_ascii_lowercase();
        let host = uri.host().unwrap_or("").to_string();
        let port = uri.port_u16().unwrap_or(if scheme == "https" { 443 } else { 80 });
        let path = uri.path_and_query().map(|p| p.as_str().to_string()).unwrap_or_else(|| "/".into());
        let method = req["method"].as_str().unwrap_or("GET").to_string();
        let req_headers = headers_from(&req["headers"]);
        let resp_headers = headers_from(&resp["headers"]);
        let req_body = req.pointer("/postData/text").and_then(|t| t.as_str()).map(|t| Bytes::from(t.to_string())).unwrap_or_default();
        let content = &resp["content"];
        let resp_body = match (content["text"].as_str(), content["encoding"].as_str()) {
            (Some(t), Some("base64")) => base64::engine::general_purpose::STANDARD.decode(t).map(Bytes::from).unwrap_or_default(),
            (Some(t), _) => Bytes::from(t.to_string()),
            _ => Bytes::new(),
        };
        // HAR bodies are already decoded: drop Content-Encoding so the inspector doesn't try to decode again
        let resp_headers: Headers = resp_headers.into_iter().filter(|(k, _)| !k.eq_ignore_ascii_case("content-encoding")).collect();
        let status = resp["status"].as_u64().filter(|s| *s > 0).map(|s| s as u16);
        let started = e["startedDateTime"].as_str().map(parse_time).unwrap_or(0.0);
        let total = ms(e.get("time"));
        let t = &e["timings"];
        let ct = header(&resp_headers, "content-type");
        let req_ct = header(&req_headers, "content-type");
        let ws: Vec<WsMessage> = e["_webSocketMessages"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|m| WsMessage {
                        at: m["time"].as_f64().unwrap_or(0.0) * 1000.0,
                        outgoing: m["type"].as_str() == Some("send"),
                        kind: if m["opcode"].as_u64() == Some(2) { "binary" } else { "text" },
                        len: m["data"].as_str().map(|d| d.len() as u64).unwrap_or(0),
                        data: m["data"].as_str().unwrap_or("").to_string(),
                        truncated: false,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let id = store.next_id();
        store.insert(Flow {
            summary: FlowSummary {
                id,
                method: method.clone(),
                scheme: scheme.clone(),
                host,
                port,
                path: path.clone(),
                http_version: req["httpVersion"].as_str().unwrap_or("HTTP/1.1").to_string(),
                status,
                state: if status.is_some() { FlowState::Complete } else { FlowState::Error },
                kind: wirecraft_core::body::classify(&method, &path, req_ct.as_deref(), ct.as_deref(), !ws.is_empty()).into(),
                content_type: ct,
                req_size: req_body.len() as u64,
                resp_size: content["size"].as_u64().unwrap_or(resp_body.len() as u64),
                started,
                duration_ms: total,
                client: Some(e["_client"].as_str().map(|s| s.to_string()).unwrap_or_else(|| label.clone())),
                client_addr: "import".into(),
                tls: scheme == "https",
                xhr: e["_resourceType"].as_str().map(|r| r == "xhr" || r == "fetch").unwrap_or(false),
                error: e["_error"].as_str().map(|s| s.to_string()),
                note: (!ws.is_empty()).then(|| format!("{} messages", ws.len())),
                req_header_count: req_headers.len(),
                resp_header_count: resp_headers.len(),
                applied: vec![],
                ws_messages: ws.len() as u64,
            },
            req_headers,
            req_body,
            req_body_truncated: false,
            status_text: resp["statusText"].as_str().filter(|s| !s.is_empty()).map(|s| s.to_string()),
            resp_http_version: resp["httpVersion"].as_str().map(|s| s.to_string()),
            resp_headers,
            resp_body,
            resp_body_truncated: false,
            resp_trailers: vec![],
            timing: Timing {
                started,
                request_ms: ms(t.get("send")),
                waiting_ms: ms(t.get("wait")),
                download_ms: ms(t.get("receive")),
                total_ms: total,
            },
            tls: None,
            ws,
        });
        n += 1;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_then_import_round_trips() {
        let doc = json!({ "log": { "version": "1.2", "entries": [{
            "startedDateTime": "2026-11-13T08:00:00.000Z",
            "time": 120.5,
            "request": { "method": "POST", "url": "https://api.example.com:8443/v1/x?a=1", "httpVersion": "h2",
                "headers": [{ "name": "content-type", "value": "application/json" }],
                "postData": { "mimeType": "application/json", "text": "{\"a\":1}" } },
            "response": { "status": 201, "statusText": "Created", "httpVersion": "h2",
                "headers": [{ "name": "content-type", "value": "application/json" }, { "name": "content-encoding", "value": "gzip" }],
                "content": { "size": 7, "mimeType": "application/json", "text": "e30=", "encoding": "base64" } },
            "timings": { "send": 1, "wait": 100, "receive": 19.5 }
        }]}});
        let store = wirecraft_core::Store::new(10);
        assert_eq!(import(&store, &doc.to_string(), "/tmp/x.har").unwrap(), 1);
        let f = store.get(store.ids()[0]).unwrap();
        assert_eq!((f.summary.method.as_str(), f.summary.port, f.summary.path.as_str()), ("POST", 8443, "/v1/x?a=1"));
        assert_eq!(f.summary.status, Some(201));
        assert_eq!(&f.resp_body[..], b"{}");
        assert!(f.resp_headers.iter().all(|(k, _)| k != "content-encoding"));
        assert_eq!(f.summary.client.as_deref(), Some("x.har"));
        // and back out again
        let out = build(&[f], "test");
        assert_eq!(out["log"]["entries"][0]["request"]["url"], "https://api.example.com:8443/v1/x?a=1");
        assert_eq!(out["log"]["entries"][0]["response"]["status"], 201);
    }
}
