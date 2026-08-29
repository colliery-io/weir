//! Self-contained WASM **TCP probe** connector ([[WEIR-T-0175]]): dial
//! `host:port` over host-brokered TCP (`fidius_guest::sockets::tcp`, gated by
//! `EgressPolicy::authorize_tcp_target`) and emit the server's FIRST bytes as
//! one record. Server-speaks-first protocols (MySQL's handshake greeting) make
//! this a real wire-level egress proof with no protocol client in the guest.
//!
//! Config: `{"host": "...", "port": N}`. Dialing by NAME exercises fidius's
//! resolve-and-pin (the policy sees `TcpTarget { host: Some(name) }`); dialing
//! an IP literal arrives as `host: None` (a name-keyed allow-list must deny it).

use fidius_macro::{plugin_impl, plugin_interface, WitType};
use weir_connector_types as wct;

// The WIT contract lives in one canonical module ([[WEIR-I-0031]]).
mod weir_guest_types;
use weir_guest_types::*;

// ---- The contract interface (re-declared against fidius_guest; same
// signatures as weir-connector's Connector ⇒ same interface_hash) ----

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

pub struct TcpProbe {
    cfg: wct::Config,
}

/// Connect and read the server's first bytes; hex-encoded so the harness can
/// assert on the exact wire greeting.
fn probe(cfg_json: &str) -> Result<String, String> {
    let v: serde_json::Value = serde_json::from_str(cfg_json).map_err(|e| e.to_string())?;
    let host = v
        .get("host")
        .and_then(|h| h.as_str())
        .ok_or("config needs `host`")?;
    let port = v
        .get("port")
        .and_then(|p| p.as_u64())
        .ok_or("config needs `port`")? as u16;
    let mut s = fidius_guest::sockets::tcp::connect(host, port)
        .map_err(|e| format!("connect {host}:{port}: {e}"))?;
    let _ = s.set_read_timeout(Some(std::time::Duration::from_secs(10)));
    let mut buf = [0u8; 512];
    let n = std::io::Read::read(&mut s, &mut buf).map_err(|e| format!("read: {e}"))?;
    Ok(buf[..n].iter().map(|b| format!("{b:02x}")).collect())
}

#[plugin_impl(Connector, crate = "fidius_guest", config = wct::Config)]
impl Connector for TcpProbe {
    fn spec(&self) -> ConnectorSpec {
        ConnectorSpec {
            name: "tcp-probe".to_string(),
            connector_version: "0.1.0".to_string(),
            contract_version: 1,
            config_schema: "{\"type\":\"object\",\"required\":[\"host\",\"port\"],\
                            \"properties\":{\"host\":{\"type\":\"string\"},\
                            \"port\":{\"type\":\"integer\"}}}"
                .to_string(),
            roles: vec![ConnectorRole::Source],
            supported_sync_modes: vec![SyncMode::FullRefresh],
        }
    }

    fn check(&self, _config: Config) -> CheckResult {
        match probe(&self.cfg.json) {
            Ok(_) => CheckResult {
                success: true,
                message: None,
            },
            Err(e) => CheckResult {
                success: false,
                message: Some(e),
            },
        }
    }

    fn discover(&self, _config: Config) -> DiscoverOutcome {
        DiscoverOutcome::Catalog(Catalog {
            streams: vec![StreamInfo {
                name: "probe".to_string(),
                namespace: None,
                schema: ArrowSchemaIpc { ipc: Vec::new() },
                supported_sync_modes: vec![SyncMode::FullRefresh],
                source_defined_cursor: false,
                default_cursor_field: None,
                source_defined_primary_key: None,
                partitioning: PartitionScheme::Unpartitioned,
            }],
        })
    }

    /// One record carrying the server's first bytes, then a checkpoint; a
    /// connect/read failure (incl. a policy denial) is a transient Fatal.
    fn read(&self, _ctx: ReadContext) -> fidius_guest::Stream<ReadMessage> {
        let msgs = match probe(&self.cfg.json) {
            Ok(hex) => vec![
                ReadMessage::Records(RecordBatch::Rows(vec![format!(
                    "{{\"first_bytes_hex\":\"{hex}\"}}"
                )])),
                ReadMessage::Checkpoint(StreamState {
                    cursor: None,
                    opaque: Vec::new(),
                }),
            ],
            Err(e) => vec![ReadMessage::Fatal(ConnectorError::transient(e))],
        };
        fidius_guest::Stream::from_iter(msgs)
    }

    fn write(
        &self,
        _ctx: WriteContext,
        mut batches: fidius_guest::Stream<RecordBatch>,
    ) -> WriteOutcome {
        while batches.next_item().is_some() {}
        WriteOutcome {
            state: StreamState {
                cursor: None,
                opaque: Vec::new(),
            },
            diagnostics: Vec::new(),
            dead_letters: Vec::new(),
            result: WriteResult::Err(ConnectorError::fatal("tcp-probe is a source")),
        }
    }
}

impl TcpProbe {
    fn configure(cfg: wct::Config) -> Self {
        Self { cfg }
    }
}
