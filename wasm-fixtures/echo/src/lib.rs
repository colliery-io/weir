//! Self-contained WASM **echo** connector (WEIR-T-0002 proof), v1 streaming.
//!
//! The typed-method types are defined **locally** (so `fidius_build::emit_wit`
//! can project them to WIT — it only reads this crate's source) and mirror
//! `weir-connector-types` exactly, so the `interface_hash` and Value shapes
//! match the host. The raw `write` body reuses `weir-connector-types` for the
//! envelope types + bincode codec; `config` is bound at construction.
//!
//! This mirrors what the connector codegen ([[WEIR-A-0029]]) emits per-connector.

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

/// `config` is bound at construction (configured instance; echo ignores it).
pub struct Echo {
    _cfg: wct::Config,
}

#[plugin_impl(Connector, crate = "fidius_guest", config = wct::Config)]
impl Connector for Echo {
    fn spec(&self) -> ConnectorSpec {
        ConnectorSpec {
            name: "echo".to_string(),
            connector_version: "0.1.0".to_string(),
            contract_version: 1,
            config_schema: "{\"type\":\"object\"}".to_string(),
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
                name: "echo".to_string(),
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

    /// Stream one fixed row, then a checkpoint.
    fn read(&self, _ctx: ReadContext) -> fidius_guest::Stream<ReadMessage> {
        fidius_guest::Stream::from_iter(vec![
            ReadMessage::Records(RecordBatch::Rows(vec!["{\"echo\":true}".to_string()])),
            ReadMessage::Checkpoint(StreamState {
                cursor: None,
                opaque: Vec::new(),
            }),
        ])
    }

    /// Client-streaming: pull every batch, counting the rows accepted.
    fn write(
        &self,
        _ctx: WriteContext,
        mut batches: fidius_guest::Stream<RecordBatch>,
    ) -> WriteOutcome {
        let mut accepted = 0u64;
        while let Some(batch) = batches.next_item() {
            if let RecordBatch::Rows(rows) = batch {
                accepted += rows.len() as u64;
            }
        }
        WriteOutcome {
            state: StreamState {
                cursor: None,
                opaque: Vec::new(),
            },
            diagnostics: Vec::new(),
            dead_letters: Vec::new(),
            result: WriteResult::Ok(WriteReceipt { accepted }),
        }
    }
}

impl Echo {
    fn configure(cfg: wct::Config) -> Self {
        Self { _cfg: cfg }
    }
}
