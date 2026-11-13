//! WebSocket message capture: a streaming RFC 6455 frame parser fed from the bytes the proxy splices between
//! client and server. Frames are passed through untouched; the parser only observes them.
//!
//! The proxy strips `Sec-WebSocket-Extensions` from upgrade requests so permessage-deflate is not negotiated and
//! payloads stay readable.

use base64::Engine;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::store::WsMessage;

/// Keep at most this much of each message's payload.
pub const DATA_CAP: usize = 64 * 1024;
/// Keep at most this many messages per connection.
pub const MAX_MESSAGES: usize = 5000;

enum State {
    /// Collecting header bytes.
    Header,
    /// Reading `remaining` payload bytes of the current frame.
    Payload { remaining: u64, mask: Option<[u8; 4]>, offset: usize, control: Option<u8> },
}

pub struct Parser {
    outgoing: bool,
    state: State,
    hdr: Vec<u8>,
    /// Data message being assembled across continuation frames: (opcode, payload, total length).
    msg: Option<(u8, Vec<u8>, u64)>,
    /// Payload of the control frame being read (they can't be fragmented and are ≤ 125 bytes).
    ctrl: Vec<u8>,
    fin: bool,
    now: fn() -> f64,
}

fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0)
}

impl Parser {
    pub fn new(outgoing: bool) -> Self {
        Self { outgoing, state: State::Header, hdr: Vec::with_capacity(14), msg: None, ctrl: Vec::new(), fin: false, now: now_ms }
    }

    fn header_len(h: &[u8]) -> Option<usize> {
        if h.len() < 2 {
            return None;
        }
        let masked = h[1] & 0x80 != 0;
        let base = match h[1] & 0x7f {
            126 => 4,
            127 => 10,
            _ => 2,
        };
        Some(base + if masked { 4 } else { 0 })
    }

    fn message(&self, opcode: u8, payload: &[u8], total: u64) -> WsMessage {
        let kind = match opcode {
            1 => "text",
            2 => "binary",
            8 => "close",
            9 => "ping",
            10 => "pong",
            _ => "binary",
        };
        let truncated = total as usize > payload.len();
        let data = match opcode {
            1 => String::from_utf8_lossy(payload).into_owned(),
            8 if payload.len() >= 2 => {
                let code = u16::from_be_bytes([payload[0], payload[1]]);
                let reason = String::from_utf8_lossy(&payload[2..]);
                if reason.is_empty() { code.to_string() } else { format!("{code} {reason}") }
            }
            9 | 10 => String::from_utf8_lossy(payload).into_owned(),
            _ => base64::engine::general_purpose::STANDARD.encode(payload),
        };
        WsMessage { at: (self.now)(), outgoing: self.outgoing, kind, len: total, data, truncated }
    }

    /// Feed bytes; returns the messages completed by them.
    pub fn feed(&mut self, mut data: &[u8]) -> Vec<WsMessage> {
        let mut out = Vec::new();
        while !data.is_empty() {
            match &mut self.state {
                State::Header => {
                    // read the 2 fixed bytes, then however many the length/mask need
                    let need = Self::header_len(&self.hdr).unwrap_or(2);
                    let take = (need - self.hdr.len()).min(data.len());
                    self.hdr.extend_from_slice(&data[..take]);
                    data = &data[take..];
                    let Some(full) = Self::header_len(&self.hdr) else { continue };
                    if self.hdr.len() < full {
                        continue;
                    }
                    let h = std::mem::take(&mut self.hdr);
                    let fin = h[0] & 0x80 != 0;
                    let opcode = h[0] & 0x0f;
                    let masked = h[1] & 0x80 != 0;
                    let (len, at) = match h[1] & 0x7f {
                        126 => (u16::from_be_bytes([h[2], h[3]]) as u64, 4),
                        127 => (u64::from_be_bytes(h[2..10].try_into().unwrap()), 10),
                        n => (n as u64, 2),
                    };
                    let mask = masked.then(|| [h[at], h[at + 1], h[at + 2], h[at + 3]]);
                    let control = (opcode >= 8).then_some(opcode);
                    if control.is_none() {
                        self.fin = fin;
                        if opcode != 0 || self.msg.is_none() {
                            self.msg = Some((if opcode == 0 { 2 } else { opcode }, Vec::new(), 0));
                        }
                    } else {
                        self.ctrl.clear();
                    }
                    self.state = State::Payload { remaining: len, mask, offset: 0, control };
                    if len == 0 {
                        self.finish_frame(&mut out);
                    }
                }
                State::Payload { remaining, mask, offset, control } => {
                    let take = (*remaining).min(data.len() as u64) as usize;
                    let chunk = &data[..take];
                    let sink: &mut Vec<u8> = if control.is_some() {
                        &mut self.ctrl
                    } else {
                        let m = self.msg.as_mut().expect("data frame has a message");
                        m.2 += take as u64;
                        &mut m.1
                    };
                    let room = DATA_CAP.saturating_sub(sink.len()).min(chunk.len());
                    let start = sink.len();
                    sink.extend_from_slice(&chunk[..room]);
                    if let Some(k) = mask {
                        for (i, b) in sink[start..].iter_mut().enumerate() {
                            *b ^= k[(*offset + i) % 4];
                        }
                    }
                    *offset += take;
                    *remaining -= take as u64;
                    data = &data[take..];
                    if *remaining == 0 {
                        self.finish_frame(&mut out);
                    }
                }
            }
        }
        out
    }

    fn finish_frame(&mut self, out: &mut Vec<WsMessage>) {
        let control = match &self.state {
            State::Payload { control, .. } => *control,
            State::Header => None,
        };
        self.state = State::Header;
        if let Some(op) = control {
            let ctrl = std::mem::take(&mut self.ctrl);
            out.push(self.message(op, &ctrl, ctrl.len() as u64));
        } else if self.fin
            && let Some((op, payload, total)) = self.msg.take() {
                out.push(self.message(op, &payload, total));
            }
    }
}

/// Copy `r` → `w`, reporting each complete WebSocket message to `on`. Returns the bytes copied.
pub async fn pump<R, W>(mut r: R, mut w: W, mut parser: Parser, on: impl Fn(Vec<WsMessage>)) -> std::io::Result<u64>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buf = vec![0u8; 16 * 1024];
    let mut total = 0u64;
    loop {
        let n = r.read(&mut buf).await?;
        if n == 0 {
            let _ = w.shutdown().await;
            return Ok(total);
        }
        w.write_all(&buf[..n]).await?;
        total += n as u64;
        let msgs = parser.feed(&buf[..n]);
        if !msgs.is_empty() {
            on(msgs);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(fin: bool, op: u8, payload: &[u8], mask: Option<[u8; 4]>) -> Vec<u8> {
        let mut v = vec![(if fin { 0x80 } else { 0 }) | op];
        let m = if mask.is_some() { 0x80 } else { 0 };
        if payload.len() < 126 {
            v.push(m | payload.len() as u8);
        } else {
            v.push(m | 126);
            v.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        }
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

    #[test]
    fn parses_masked_text_split_across_reads() {
        let bytes = frame(true, 1, b"hello world", Some([1, 2, 3, 4]));
        let mut p = Parser::new(true);
        let mut got = vec![];
        for c in bytes.chunks(3) {
            got.extend(p.feed(c));
        }
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].data, "hello world");
        assert!(got[0].outgoing);
    }

    #[test]
    fn fragments_and_interleaved_ping() {
        let mut bytes = frame(false, 1, b"ab", None);
        bytes.extend(frame(true, 9, b"p", None));
        bytes.extend(frame(true, 0, b"cd", None));
        bytes.extend(frame(true, 8, &[0x03, 0xe8, b'b', b'y', b'e'], None));
        let got = Parser::new(false).feed(&bytes);
        let v: Vec<_> = got.iter().map(|m| (m.kind, m.data.as_str())).collect();
        assert_eq!(v, vec![("ping", "p"), ("text", "abcd"), ("close", "1000 bye")]);
    }

    #[test]
    fn long_payload_length() {
        let payload = vec![b'x'; 300];
        let got = Parser::new(false).feed(&frame(true, 1, &payload, None));
        assert_eq!(got[0].len, 300);
    }
}
