//! Body helpers: a tee body that records what streams through it, content-encoding decoding, and
//! resource-kind classification for the table's Type column and quick chips.

use std::io::Read;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Instant;

use bytes::{Bytes, BytesMut};
use hyper::body::{Body, Frame, SizeHint};

/// Called once when a tee'd body finishes (or is dropped early).
pub struct TeeDone {
    pub captured: Bytes,
    pub truncated: bool,
    pub total: u64,
    pub elapsed_ms: f64,
    pub error: Option<String>,
}

pin_project_lite::pin_project! {
    /// Passes frames through unchanged while keeping a copy of the first `cap` bytes.
    pub struct TeeBody<B> {
        #[pin]
        inner: B,
        buf: BytesMut,
        cap: usize,
        total: u64,
        truncated: bool,
        start: Instant,
        done: Option<Box<dyn FnOnce(TeeDone) + Send + 'static>>,
    }

    impl<B> PinnedDrop for TeeBody<B> {
        fn drop(this: Pin<&mut Self>) {
            let this = this.project();
            if let Some(done) = this.done.take() {
                done(TeeDone {
                    captured: std::mem::take(this.buf).freeze(),
                    truncated: *this.truncated,
                    total: *this.total,
                    elapsed_ms: this.start.elapsed().as_secs_f64() * 1000.0,
                    error: Some("client closed the connection before the body finished".into()),
                });
            }
        }
    }
}

impl<B> TeeBody<B> {
    pub fn new(inner: B, cap: usize, done: impl FnOnce(TeeDone) + Send + 'static) -> Self {
        Self {
            inner,
            buf: BytesMut::new(),
            cap,
            total: 0,
            truncated: false,
            start: Instant::now(),
            done: Some(Box::new(done)),
        }
    }
}

impl<B> Body for TeeBody<B>
where
    B: Body<Data = Bytes>,
    B::Error: std::fmt::Display,
{
    type Data = Bytes;
    type Error = B::Error;

    fn poll_frame(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Result<Frame<Bytes>, B::Error>>> {
        let mut this = self.project();
        let res = this.inner.as_mut().poll_frame(cx);
        match &res {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(d) = frame.data_ref() {
                    *this.total += d.len() as u64;
                    let room = this.cap.saturating_sub(this.buf.len());
                    if room >= d.len() {
                        this.buf.extend_from_slice(d);
                    } else {
                        this.buf.extend_from_slice(&d[..room]);
                        *this.truncated = true;
                    }
                }
                // hyper stops polling once the body reports end-of-stream, so finish here too
                if this.inner.is_end_stream() {
                    if let Some(done) = this.done.take() {
                        done(TeeDone {
                            captured: std::mem::take(this.buf).freeze(),
                            truncated: *this.truncated,
                            total: *this.total,
                            elapsed_ms: this.start.elapsed().as_secs_f64() * 1000.0,
                            error: None,
                        });
                    }
                }
            }
            Poll::Ready(other) => {
                let error = match other {
                    Some(Err(e)) => Some(e.to_string()),
                    _ => None,
                };
                if let Some(done) = this.done.take() {
                    done(TeeDone {
                        captured: std::mem::take(this.buf).freeze(),
                        truncated: *this.truncated,
                        total: *this.total,
                        elapsed_ms: this.start.elapsed().as_secs_f64() * 1000.0,
                        error,
                    });
                }
            }
            Poll::Pending => {}
        }
        res
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

/// Undo `Content-Encoding` (gzip, deflate, br, zstd; chained encodings are applied in reverse).
pub fn decode(body: &[u8], encoding: Option<&str>) -> Result<Vec<u8>, String> {
    let Some(enc) = encoding else { return Ok(body.to_vec()) };
    let mut data = body.to_vec();
    for e in enc.split(',').map(|s| s.trim().to_ascii_lowercase()).rev() {
        data = match e.as_str() {
            "" | "identity" => data,
            "gzip" | "x-gzip" => read_all(flate2::read::MultiGzDecoder::new(&data[..]))?,
            "deflate" => {
                // servers disagree on zlib-wrapped vs raw deflate
                read_all(flate2::read::ZlibDecoder::new(&data[..]))
                    .or_else(|_| read_all(flate2::read::DeflateDecoder::new(&data[..])))?
            }
            "br" => read_all(brotli::Decompressor::new(&data[..], 4096))?,
            "zstd" => zstd::stream::decode_all(&data[..]).map_err(|e| e.to_string())?,
            other => return Err(format!("unsupported content-encoding: {other}")),
        };
    }
    Ok(data)
}

fn read_all(mut r: impl Read) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    r.read_to_end(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

/// Short resource type shown in the Type column: JSON, JS, CSS, Img, Media, Font, HTML, XML, GQL, gRPC, WS, Text, Bin, —.
pub fn classify(method: &str, path: &str, req_ct: Option<&str>, resp_ct: Option<&str>, upgrade: bool) -> &'static str {
    if upgrade {
        return "WS";
    }
    let ct = resp_ct.or(req_ct).unwrap_or("").to_ascii_lowercase();
    let req_ct = req_ct.unwrap_or("").to_ascii_lowercase();
    if req_ct.starts_with("application/grpc") || ct.starts_with("application/grpc") {
        return "gRPC";
    }
    let p = path.split('?').next().unwrap_or("").to_ascii_lowercase();
    if p.ends_with("/graphql") || p.contains("/graphql/") || ct.contains("graphql") {
        return "GQL";
    }
    if method == "CONNECT" {
        return "TLS";
    }
    let ext = p.rsplit('/').next().and_then(|s| s.rsplit_once('.')).map(|(_, e)| e).unwrap_or("");
    let by_ct = if ct.contains("json") {
        Some("JSON")
    } else if ct.contains("javascript") || ct.contains("ecmascript") {
        Some("JS")
    } else if ct.starts_with("text/css") {
        Some("CSS")
    } else if ct.starts_with("image/") {
        Some("Img")
    } else if ct.starts_with("video/") || ct.starts_with("audio/") || ct.contains("mpegurl") || ct.contains("dash+xml") {
        Some("Media")
    } else if ct.starts_with("font/") || ct.contains("font-") {
        Some("Font")
    } else if ct.starts_with("text/html") || ct.contains("xhtml") {
        Some("HTML")
    } else if ct.contains("xml") {
        Some("XML")
    } else if ct.starts_with("text/") {
        Some("Text")
    } else if ct.contains("wasm") {
        Some("Wasm")
    } else {
        None
    };
    if let Some(k) = by_ct {
        return k;
    }
    match ext {
        "js" | "mjs" | "cjs" => "JS",
        "css" => "CSS",
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif" | "svg" | "ico" | "bmp" => "Img",
        "mp4" | "webm" | "mp3" | "m4a" | "aac" | "ogg" | "wav" | "m3u8" | "ts" | "mpd" => "Media",
        "woff" | "woff2" | "ttf" | "otf" | "eot" => "Font",
        "html" | "htm" => "HTML",
        "json" => "JSON",
        _ if ct.is_empty() => "—",
        _ => "Bin",
    }
}
