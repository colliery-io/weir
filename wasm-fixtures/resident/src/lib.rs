//! Self-contained WASM **resident** connector ([[WEIR-I-0035]] F1.9) — one connector,
//! three modes selected by config `mode`:
//!
//! - **`poll`** (read-amplification clamp): a warm loop — each cycle emits a **bounded
//!   batch** (`rows_per_poll` rows, a "read to end" of the current synthetic dataset) +
//!   a `Checkpoint`, forever. It does **not** self-pace; the HOST sleeps the connection's
//!   declared cadence (`ExecutionMode::Resident.cadence_ms`, floor ~20ms) **between polls**
//!   (per `Checkpoint`). This is a real poll (bounded read-to-end per cadence cycle), NOT
//!   an unbounded per-row stream.
//! - **`tail`** (the "while"/event-reader shape): one `Records` per config-supplied
//!   **arrival** (`{"events":[...]}`), then a terminal `Checkpoint`. Emission tracks
//!   arrivals, not a clock: K arrivals → K rows; 0 → 0.
//! - **`ws`** (websocket pass-through, [[WEIR-A-0039]]): a genuinely *blocking* live tail —
//!   `read` emits one record per inbound frame; `write` pushes records as frames. RFC6455
//!   is hand-rolled over the host-brokered TCP egress (`fidius_guest::sockets::tcp`, gated
//!   by `EgressPolicy::authorize_tcp`) — no ws lib, no rand/base64/sha1, so it wasm-builds
//!   deterministically. `ws://` only; `wss`/auth is a host-brokered-handshake follow-on.
//!
//! Config: `{"mode":"poll"|"tail"|"ws", "rows_per_poll":N, "events":[...],
//! "read_url":"ws://…", "write_url":"ws://…", "url":"ws://…"}`. Default mode = `poll`.

use std::collections::VecDeque;
use std::io::{Read, Write};

use fidius_guest::sockets::tcp;
use fidius_macro::{plugin_impl, plugin_interface, WitType};
use weir_connector_types as wct;

mod weir_guest_types;
use weir_guest_types::*;

#[plugin_interface(version = 1, buffer = PluginAllocated, crate = "fidius_guest")]
pub trait Connector: Send + Sync {
    fn spec(&self) -> ConnectorSpec;
    fn check(&self, config: Config) -> CheckResult;

    #[optional(since = 1)]
    fn discover(&self, config: Config) -> DiscoverOutcome;

    #[optional(since = 1)]
    fn read(&self, ctx: ReadContext) -> fidius_guest::Stream<ReadMessage>;

    #[optional(since = 1)]
    fn write(&self, ctx: WriteContext, batches: fidius_guest::Stream<RecordBatch>) -> WriteOutcome;
}

// ---------------------------------------------------------------------------
// Minimal RFC6455 client over the brokered TCP stream (ws mode). Text/binary
// frames; client frames are masked (a cheap LCG mask avoids a rand/getrandom dep
// and keeps the wasm build deterministic). Fixed handshake key + lenient "101"
// check (a client need not verify Sec-WebSocket-Accept), avoiding base64/sha1.
// ---------------------------------------------------------------------------
struct Ws {
    s: tcp::TcpStream,
    mask: u32,
}

impl Ws {
    fn connect(url: &str) -> Result<Ws, String> {
        let rest = url
            .strip_prefix("ws://")
            .ok_or_else(|| format!("only ws:// supported (got `{url}`)"))?;
        let (hostport, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, "/"),
        };
        let (host, port) = hostport.split_once(':').unwrap_or((hostport, "80"));
        let port: u16 = port.parse().map_err(|_| format!("bad port in `{url}`"))?;

        let mut s = tcp::connect(host, port).map_err(|e| format!("tcp connect {host}:{port}: {e}"))?;
        let req = format!(
            "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nUpgrade: websocket\r\n\
             Connection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
             Sec-WebSocket-Version: 13\r\n\r\n"
        );
        s.write_all(req.as_bytes()).map_err(|e| format!("handshake write: {e}"))?;
        s.flush().map_err(|e| format!("handshake flush: {e}"))?;

        let mut head = Vec::new();
        let mut b = [0u8; 1];
        loop {
            let n = s.read(&mut b).map_err(|e| format!("handshake read: {e}"))?;
            if n == 0 {
                return Err("handshake: eof before headers".to_string());
            }
            head.push(b[0]);
            if head.len() >= 4 && &head[head.len() - 4..] == b"\r\n\r\n" {
                break;
            }
            if head.len() > 8192 {
                return Err("handshake: response headers too large".to_string());
            }
        }
        let status = String::from_utf8_lossy(&head);
        let line = status.lines().next().unwrap_or("");
        if !line.contains(" 101") {
            return Err(format!("handshake not 101: `{line}`"));
        }
        Ok(Ws { s, mask: 0x1234_5678 })
    }

    fn next_mask(&mut self) -> [u8; 4] {
        self.mask = self.mask.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.mask.to_be_bytes()
    }

    fn send(&mut self, opcode: u8, payload: &[u8]) -> Result<(), String> {
        let mut f = Vec::with_capacity(payload.len() + 8);
        f.push(0x80 | opcode);
        let len = payload.len();
        if len < 126 {
            f.push(0x80 | len as u8);
        } else if len < 65536 {
            f.push(0x80 | 126);
            f.extend_from_slice(&(len as u16).to_be_bytes());
        } else {
            f.push(0x80 | 127);
            f.extend_from_slice(&(len as u64).to_be_bytes());
        }
        let m = self.next_mask();
        f.extend_from_slice(&m);
        for (i, b) in payload.iter().enumerate() {
            f.push(b ^ m[i % 4]);
        }
        self.s.write_all(&f).map_err(|e| format!("frame write: {e}"))?;
        self.s.flush().map_err(|e| format!("frame flush: {e}"))
    }

    fn send_text(&mut self, text: &str) -> Result<(), String> {
        self.send(0x1, text.as_bytes())
    }

    fn read_n(&mut self, n: usize) -> Result<Vec<u8>, String> {
        let mut v = vec![0u8; n];
        let mut off = 0;
        while off < n {
            let r = self.s.read(&mut v[off..]).map_err(|e| format!("read: {e}"))?;
            if r == 0 {
                return Err("eof mid-frame".to_string());
            }
            off += r;
        }
        Ok(v)
    }

    fn read_message(&mut self) -> Result<Option<String>, String> {
        loop {
            let h = self.read_n(2)?;
            let opcode = h[0] & 0x0f;
            let masked = h[1] & 0x80 != 0;
            let mut len = (h[1] & 0x7f) as usize;
            if len == 126 {
                let e = self.read_n(2)?;
                len = u16::from_be_bytes([e[0], e[1]]) as usize;
            } else if len == 127 {
                let e = self.read_n(8)?;
                len = u64::from_be_bytes(e.as_slice().try_into().unwrap()) as usize;
            }
            let mask = if masked { Some(self.read_n(4)?) } else { None };
            let mut payload = self.read_n(len)?;
            if let Some(m) = &mask {
                for (i, b) in payload.iter_mut().enumerate() {
                    *b ^= m[i % 4];
                }
            }
            match opcode {
                0x1 | 0x2 => return Ok(Some(String::from_utf8_lossy(&payload).into_owned())),
                0x8 => return Ok(None),
                0x9 => self.send(0xA, &payload)?,
                _ => {}
            }
        }
    }
}

/// Blocking ws tail iterator: `next()` blocks on the socket; one `Records` + `Checkpoint`
/// per inbound frame. A read error / clean close ends the tail with a **transient** `Fatal`
/// so `run_resident` requeues + reconnects ([[WEIR-I-0035]] F1.3 supervision).
struct WsSource {
    ws: Option<Ws>,
    pending: VecDeque<ReadMessage>,
    done: bool,
    seq: u64,
}

impl Iterator for WsSource {
    type Item = ReadMessage;
    fn next(&mut self) -> Option<ReadMessage> {
        loop {
            if let Some(m) = self.pending.pop_front() {
                return Some(m);
            }
            if self.done {
                return None;
            }
            let Some(ws) = self.ws.as_mut() else {
                self.done = true;
                return Some(ReadMessage::Fatal(ConnectorError::transient("ws source not connected")));
            };
            match ws.read_message() {
                Ok(Some(text)) => {
                    self.seq += 1;
                    self.pending.push_back(ReadMessage::Records(RecordBatch::Rows(vec![text])));
                    self.pending.push_back(ReadMessage::Checkpoint(StreamState {
                        cursor: Some(self.seq.to_string()),
                        opaque: Vec::new(),
                    }));
                }
                Ok(None) => {
                    self.done = true;
                    return Some(ReadMessage::Fatal(ConnectorError::transient("upstream ws closed")));
                }
                Err(e) => {
                    self.done = true;
                    return Some(ReadMessage::Fatal(ConnectorError::transient(&format!("ws read: {e}"))));
                }
            }
        }
    }
}

pub struct Resident {
    cfg: wct::Config,
}

impl Resident {
    fn configure(cfg: wct::Config) -> Self {
        Self { cfg }
    }

    fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.cfg.json).unwrap_or(serde_json::Value::Null)
    }

    fn mode(&self) -> String {
        self.json()
            .get("mode")
            .and_then(|m| m.as_str())
            .unwrap_or("poll")
            .to_string()
    }

    fn url_for(&self, key: &str) -> String {
        let v = self.json();
        v.get(key)
            .or_else(|| v.get("url"))
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .to_string()
    }
}

#[plugin_impl(Connector, crate = "fidius_guest", config = wct::Config)]
impl Connector for Resident {
    fn spec(&self) -> ConnectorSpec {
        ConnectorSpec {
            name: "resident".to_string(),
            connector_version: "0.1.0".to_string(),
            contract_version: 1,
            config_schema: "{\"type\":\"object\",\"properties\":{\"mode\":{\"type\":\"string\",\"enum\":[\"poll\",\"tail\",\"ws\"]},\"rows_per_poll\":{\"type\":\"integer\"},\"events\":{\"type\":\"array\"},\"read_url\":{\"type\":\"string\"},\"write_url\":{\"type\":\"string\"},\"url\":{\"type\":\"string\"}}}".to_string(),
            roles: vec![ConnectorRole::Source, ConnectorRole::Destination],
            supported_sync_modes: vec![SyncMode::FullRefresh],
        }
    }

    fn check(&self, _config: Config) -> CheckResult {
        CheckResult { success: true, message: None }
    }

    fn discover(&self, _config: Config) -> DiscoverOutcome {
        DiscoverOutcome::Catalog(Catalog {
            streams: vec![StreamInfo {
                name: "resident".to_string(),
                namespace: None,
                schema: ArrowSchemaIpc { ipc: Vec::new() },
                supported_sync_modes: vec![SyncMode::FullRefresh],
                source_defined_cursor: true,
                default_cursor_field: None,
                source_defined_primary_key: None,
                partitioning: PartitionScheme::Unpartitioned,
            }],
        })
    }

    fn read(&self, _ctx: ReadContext) -> fidius_guest::Stream<ReadMessage> {
        match self.mode().as_str() {
            // POLL: a warm loop. Each cycle emits a BOUNDED batch (`rows_per_poll` rows —
            // a read-to-end of the current dataset) + a Checkpoint, forever. The host sleeps
            // the declared cadence between polls (per Checkpoint); the guest does not self-pace.
            "tail" => {
                let events: Vec<String> = self
                    .json()
                    .get("events")
                    .and_then(|e| e.as_array())
                    .map(|a| a.iter().map(|x| x.to_string()).collect())
                    .unwrap_or_default();
                let mut msgs: Vec<ReadMessage> = events
                    .into_iter()
                    .map(|e| ReadMessage::Records(RecordBatch::Rows(vec![e])))
                    .collect();
                msgs.push(ReadMessage::Checkpoint(StreamState { cursor: None, opaque: Vec::new() }));
                fidius_guest::Stream::from_iter(msgs)
            }
            "ws" => {
                let url = self.url_for("read_url");
                let (ws, boot) = match Ws::connect(&url) {
                    Ok(w) => (Some(w), None),
                    Err(e) => (
                        None,
                        Some(ReadMessage::Fatal(ConnectorError::transient(&format!("ws connect `{url}`: {e}")))),
                    ),
                };
                let mut pending = VecDeque::new();
                if let Some(b) = boot {
                    pending.push_back(b);
                }
                let done = ws.is_none();
                fidius_guest::Stream::from_iter(WsSource { ws, pending, done, seq: 0 })
            }
            // default → poll
            _ => {
                let n = self
                    .json()
                    .get("rows_per_poll")
                    .and_then(|r| r.as_u64())
                    .unwrap_or(3)
                    .max(1);
                fidius_guest::Stream::from_iter((0u64..).flat_map(move |cycle| {
                    let rows: Vec<String> = (0..n)
                        .map(|r| format!("{{\"cycle\":{cycle},\"row\":{r}}}"))
                        .collect();
                    vec![
                        ReadMessage::Records(RecordBatch::Rows(rows)),
                        ReadMessage::Checkpoint(StreamState {
                            cursor: Some(cycle.to_string()),
                            opaque: Vec::new(),
                        }),
                    ]
                }))
            }
        }
    }

    fn write(
        &self,
        _ctx: WriteContext,
        mut batches: fidius_guest::Stream<RecordBatch>,
    ) -> WriteOutcome {
        // Only `ws` mode is a destination (push each record as a frame). Other modes are
        // sources — drain + report a fatal (they should never be wired as a dest).
        if self.mode() == "ws" {
            let url = self.url_for("write_url");
            let mut ws = match Ws::connect(&url) {
                Ok(w) => w,
                Err(e) => {
                    return WriteOutcome {
                        state: StreamState { cursor: None, opaque: Vec::new() },
                        diagnostics: Vec::new(),
                        dead_letters: Vec::new(),
                        result: WriteResult::Err(ConnectorError::transient(&format!("ws connect `{url}`: {e}"))),
                    }
                }
            };
            let mut accepted: u64 = 0;
            let mut dead: Vec<DeadLetter> = Vec::new();
            while let Some(batch) = batches.next_item() {
                if let RecordBatch::Rows(rows) = batch {
                    for r in rows {
                        match ws.send_text(&r) {
                            Ok(()) => accepted += 1,
                            Err(e) => dead.push(DeadLetter { record: r, reason: e }),
                        }
                    }
                }
            }
            return WriteOutcome {
                state: StreamState { cursor: None, opaque: Vec::new() },
                diagnostics: Vec::new(),
                dead_letters: dead,
                result: WriteResult::Ok(WriteReceipt { accepted }),
            };
        }
        while batches.next_item().is_some() {}
        WriteOutcome {
            state: StreamState { cursor: None, opaque: Vec::new() },
            diagnostics: Vec::new(),
            dead_letters: Vec::new(),
            result: WriteResult::Err(ConnectorError::fatal("resident mode is a source, not a destination")),
        }
    }
}
