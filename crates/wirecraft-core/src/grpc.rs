//! gRPC / gRPC-Web framing and schemaless protobuf decoding, for the inspector.
//!
//! Without the .proto files only wire types are known, so length-delimited fields are shown as a nested
//! message when they parse as one, as text when they are printable UTF-8, and as bytes otherwise.

use std::io::Read;

use base64::Engine;
use serde_json::{Value, json};

const MAX_DEPTH: usize = 24;
const MAX_MESSAGES: usize = 500;

/// What kind of protobuf payload a content type carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `application/grpc[+proto]`: 5-byte length-prefixed messages.
    Grpc,
    /// `application/grpc-web[+proto]`: like gRPC, trailers in a frame flagged 0x80.
    GrpcWeb,
    /// `application/grpc-web-text`: base64 of gRPC-Web.
    GrpcWebText,
    /// `application/x-protobuf`, `application/protobuf`, `application/vnd.google.protobuf`: one bare message.
    Protobuf,
}

pub fn kind(content_type: &str) -> Option<Kind> {
    let ct = content_type.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    if ct.starts_with("application/grpc-web-text") {
        Some(Kind::GrpcWebText)
    } else if ct.starts_with("application/grpc-web") {
        (!ct.contains("json")).then_some(Kind::GrpcWeb)
    } else if ct.starts_with("application/grpc") {
        (!ct.contains("json")).then_some(Kind::Grpc)
    } else if matches!(ct.as_str(), "application/x-protobuf" | "application/protobuf" | "application/vnd.google.protobuf" | "application/x-google-protobuf")
        || ct.ends_with("+proto")
        || ct.ends_with("+protobuf")
    {
        Some(Kind::Protobuf)
    } else {
        None
    }
}

/// Decode a body for display. `encoding` is the `grpc-encoding` header (for compressed messages).
///
/// Returns `{ kind, messages: [{ compressed, size, fields | error }], trailers? }`, or `None` when the content
/// type is not protobuf.
pub fn decode_body(body: &[u8], content_type: &str, encoding: Option<&str>) -> Option<Value> {
    let kind = kind(content_type)?;
    if body.is_empty() {
        return Some(json!({ "kind": kind_name(kind), "messages": [] }));
    }
    let owned;
    let body = if kind == Kind::GrpcWebText {
        let clean: Vec<u8> = body.iter().copied().filter(|b| !b.is_ascii_whitespace()).collect();
        // gRPC-Web-Text may concatenate separately padded base64 chunks
        let mut out = Vec::new();
        for chunk in clean.split_inclusive(|b| *b == b'=').filter(|c| !c.iter().all(|b| *b == b'=')) {
            match base64::engine::general_purpose::STANDARD.decode(chunk) {
                Ok(d) => out.extend(d),
                Err(_) => return Some(json!({ "kind": kind_name(kind), "error": "invalid base64" })),
            }
        }
        owned = out;
        &owned[..]
    } else {
        body
    };
    if kind == Kind::Protobuf {
        return Some(match decode_message(body, 0) {
            Some(fields) => json!({ "kind": "protobuf", "messages": [{ "compressed": false, "size": body.len(), "fields": fields }] }),
            None => json!({ "kind": "protobuf", "error": "not a valid protobuf message" }),
        });
    }
    let mut messages = Vec::new();
    let mut trailers = None;
    let mut rest = body;
    while !rest.is_empty() && messages.len() < MAX_MESSAGES {
        if rest.len() < 5 {
            messages.push(json!({ "error": format!("{} trailing bytes (truncated frame)", rest.len()) }));
            break;
        }
        let flag = rest[0];
        let len = u32::from_be_bytes([rest[1], rest[2], rest[3], rest[4]]) as usize;
        let Some(payload) = rest.get(5..5 + len) else {
            messages.push(json!({ "compressed": flag & 1 == 1, "size": len, "error": format!("frame says {len} bytes, {} present (body clipped?)", rest.len() - 5) }));
            break;
        };
        rest = &rest[5 + len..];
        if flag & 0x80 != 0 {
            // gRPC-Web trailers: HTTP/1-style header lines
            let t: serde_json::Map<String, Value> = String::from_utf8_lossy(payload)
                .lines()
                .filter_map(|l| l.split_once(':').map(|(k, v)| (k.trim().to_ascii_lowercase(), Value::String(v.trim().to_string()))))
                .collect();
            trailers = Some(Value::Object(t));
            continue;
        }
        let compressed = flag & 1 == 1;
        let data = if compressed {
            match decompress(payload, encoding.unwrap_or("gzip")) {
                Ok(d) => d,
                Err(e) => {
                    messages.push(json!({ "compressed": true, "size": len, "error": e }));
                    continue;
                }
            }
        } else {
            payload.to_vec()
        };
        messages.push(match decode_message(&data, 0) {
            Some(fields) => json!({ "compressed": compressed, "size": len, "fields": fields }),
            None => json!({ "compressed": compressed, "size": len, "error": "not a valid protobuf message", "hex": hex(&data[..data.len().min(256)]) }),
        });
    }
    let mut v = json!({ "kind": kind_name(kind), "messages": messages });
    if let Some(t) = trailers {
        v["trailers"] = t;
    }
    Some(v)
}

fn kind_name(k: Kind) -> &'static str {
    match k {
        Kind::Grpc => "grpc",
        Kind::GrpcWeb => "grpc-web",
        Kind::GrpcWebText => "grpc-web-text",
        Kind::Protobuf => "protobuf",
    }
}

fn decompress(data: &[u8], encoding: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    match encoding.trim().to_ascii_lowercase().as_str() {
        "gzip" => flate2::read::GzDecoder::new(data).read_to_end(&mut out).map_err(|e| e.to_string())?,
        "deflate" => flate2::read::ZlibDecoder::new(data).read_to_end(&mut out).map_err(|e| e.to_string())?,
        "zstd" => return zstd::decode_all(data).map_err(|e| e.to_string()),
        other => return Err(format!("unsupported grpc-encoding {other}")),
    };
    Ok(out)
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ")
}

fn varint(b: &[u8], pos: &mut usize) -> Option<u64> {
    let mut v: u64 = 0;
    for i in 0..10 {
        let byte = *b.get(*pos)?;
        *pos += 1;
        v |= ((byte & 0x7f) as u64) << (7 * i);
        if byte & 0x80 == 0 {
            return Some(v);
        }
    }
    None
}

/// Decode a message as a list of `{ no, wire, value, … }` fields; `None` if the bytes aren't a valid message.
pub fn decode_message(b: &[u8], depth: usize) -> Option<Vec<Value>> {
    if depth > MAX_DEPTH {
        return None;
    }
    let mut pos = 0;
    let mut fields = Vec::new();
    while pos < b.len() {
        let key = varint(b, &mut pos)?;
        let no = key >> 3;
        if no == 0 || no > 536_870_911 {
            return None;
        }
        let f = match key & 7 {
            0 => {
                let v = varint(b, &mut pos)?;
                let zz = ((v >> 1) as i64) ^ -((v & 1) as i64);
                let mut f = json!({ "no": no, "wire": "varint", "value": v });
                if (v as i64) < 0 {
                    f["int64"] = json!(v as i64);
                }
                if zz != v as i64 {
                    f["sint"] = json!(zz);
                }
                f
            }
            1 => {
                let raw: [u8; 8] = b.get(pos..pos + 8)?.try_into().ok()?;
                pos += 8;
                let u = u64::from_le_bytes(raw);
                let d = f64::from_le_bytes(raw);
                let mut f = json!({ "no": no, "wire": "fixed64", "value": u });
                if d.is_finite() && d != 0.0 && d.abs() > 1e-300 && d.abs() < 1e300 {
                    f["double"] = json!(d);
                }
                f
            }
            2 => {
                let len = varint(b, &mut pos)? as usize;
                let data = b.get(pos..pos.checked_add(len)?)?;
                pos += len;
                length_delimited(no, data, depth)
            }
            5 => {
                let raw: [u8; 4] = b.get(pos..pos + 4)?.try_into().ok()?;
                pos += 4;
                let u = u32::from_le_bytes(raw);
                let fl = f32::from_le_bytes(raw);
                let mut f = json!({ "no": no, "wire": "fixed32", "value": u });
                if fl.is_finite() && fl != 0.0 && fl.abs() > 1e-30 && fl.abs() < 1e30 {
                    f["float"] = json!(fl);
                }
                f
            }
            // groups (3/4) are deprecated and rare; treat as invalid
            _ => return None,
        };
        fields.push(f);
    }
    Some(fields)
}

fn printable(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| !c.is_control() || c == '\n' || c == '\r' || c == '\t')
}

fn length_delimited(no: u64, data: &[u8], depth: usize) -> Value {
    if data.is_empty() {
        return json!({ "no": no, "wire": "len", "type": "string", "value": "" });
    }
    let text = std::str::from_utf8(data).ok().filter(|s| printable(s));
    // short printable text is far more likely a string than a message that happens to parse
    let prefer_text = text.map(|s| s.len() < 4 || s.chars().all(|c| c.is_ascii_graphic() || c == ' ')).unwrap_or(false);
    if !prefer_text
        && let Some(fields) = decode_message(data, depth + 1).filter(|f| !f.is_empty()) {
            return json!({ "no": no, "wire": "len", "type": "message", "size": data.len(), "value": fields });
        }
    if let Some(s) = text {
        return json!({ "no": no, "wire": "len", "type": "string", "value": s });
    }
    // packed varints are common for repeated numeric fields
    let mut pos = 0;
    let mut packed = Vec::new();
    while pos < data.len() {
        match varint(data, &mut pos) {
            Some(v) => packed.push(v),
            None => {
                packed.clear();
                break;
            }
        }
    }
    let mut f = json!({ "no": no, "wire": "len", "type": "bytes", "size": data.len(), "value": base64::engine::general_purpose::STANDARD.encode(data) });
    if !packed.is_empty() && packed.len() <= 1000 {
        f["packed"] = json!(packed);
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    // message { int32 a = 1; string b = 2; Inner c = 3; } Inner { sint32 x = 1; double d = 2; }
    fn sample() -> Vec<u8> {
        let mut inner = vec![0x08, 0x03]; // x = sint -2
        inner.push(0x11);
        inner.extend(1.5f64.to_le_bytes());
        let mut m = vec![0x08, 0x96, 0x01]; // a = 150
        m.extend([0x12, 0x07]);
        m.extend(b"testing");
        m.push(0x1a);
        m.push(inner.len() as u8);
        m.extend(inner);
        m
    }

    #[test]
    fn decodes_bare_message() {
        let v = decode_body(&sample(), "application/x-protobuf", None).unwrap();
        let f = &v["messages"][0]["fields"];
        assert_eq!(f[0]["value"], 150);
        assert_eq!(f[1]["value"], "testing");
        assert_eq!(f[2]["type"], "message");
        assert_eq!(f[2]["value"][0]["sint"], -2);
        assert_eq!(f[2]["value"][1]["double"], 1.5);
    }

    #[test]
    fn decodes_grpc_frames_and_gzip() {
        let m = sample();
        let mut body = vec![0u8];
        body.extend((m.len() as u32).to_be_bytes());
        body.extend(&m);
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        std::io::Write::write_all(&mut gz, &m).unwrap();
        let gz = gz.finish().unwrap();
        body.push(1);
        body.extend((gz.len() as u32).to_be_bytes());
        body.extend(&gz);
        let v = decode_body(&body, "application/grpc", Some("gzip")).unwrap();
        assert_eq!(v["messages"].as_array().unwrap().len(), 2);
        assert_eq!(v["messages"][1]["compressed"], true);
        assert_eq!(v["messages"][1]["fields"][1]["value"], "testing");
    }

    #[test]
    fn grpc_web_text_with_trailers() {
        let m = sample();
        let mut body = vec![0u8];
        body.extend((m.len() as u32).to_be_bytes());
        body.extend(&m);
        let t = b"grpc-status: 0\r\ngrpc-message: OK\r\n";
        body.push(0x80);
        body.extend((t.len() as u32).to_be_bytes());
        body.extend(t);
        let b64 = base64::engine::general_purpose::STANDARD.encode(&body);
        let v = decode_body(b64.as_bytes(), "application/grpc-web-text+proto", None).unwrap();
        assert_eq!(v["trailers"]["grpc-status"], "0");
        assert_eq!(v["messages"][0]["fields"][0]["value"], 150);
    }

    #[test]
    fn rejects_garbage_and_other_types() {
        assert!(decode_body(b"{}", "application/json", None).is_none());
        assert!(decode_body(b"{}", "application/grpc+json", None).is_none());
        let v = decode_body(&[0xff, 0xff, 0xff], "application/x-protobuf", None).unwrap();
        assert!(v["error"].is_string());
    }
}
