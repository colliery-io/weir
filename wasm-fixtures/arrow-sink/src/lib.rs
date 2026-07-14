//! Hand-written WASM guest for the `arrow-sink` connector (WEIR-I-0011 S2): a pure
//! destination that accepts the bulk Arrow path (decode IPC → count rows) + the
//! Rows path, proving a **wasm destination** + client-streaming `write` over wasm.
//! The `weir_guest_types` block + `Connector` trait are verbatim (the contract).
#![allow(clippy::all)]

use std::io::Cursor;

use fidius_macro::{plugin_impl, plugin_interface, WitType};
use weir_guest_types::*;

mod weir_guest_types;

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

pub struct ArrowSink { _cfg: weir_connector_types::Config }

#[plugin_impl(Connector, crate = "fidius_guest", config = weir_connector_types::Config)]
impl Connector for ArrowSink {
    fn spec(&self) -> ConnectorSpec {
        ConnectorSpec {
            name: "arrow-sink".to_string(),
            connector_version: "0.1.0".to_string(),
            contract_version: 1,
            config_schema: "{\"type\":\"object\"}".to_string(),
            roles: vec![ConnectorRole::Destination],
            supported_sync_modes: vec![SyncMode::FullRefresh],
        }
    }

    fn check(&self, _config: Config) -> CheckResult {
        CheckResult { success: true, message: None }
    }

    fn discover(&self, _config: Config) -> DiscoverOutcome {
        DiscoverOutcome::Error(ConnectorError::fatal("arrow-sink is a destination, not a source"))
    }

    fn read(&self, _ctx: ReadContext) -> fidius_guest::Stream<ReadMessage> {
        fidius_guest::Stream::from_iter(Vec::<ReadMessage>::new())
    }

    /// Client-streaming: pull each batch, decode Arrow IPC (or count Rows), sum rows.
    fn write(&self, _ctx: WriteContext, mut batches: fidius_guest::Stream<RecordBatch>) -> WriteOutcome {
        let mut accepted = 0u64;
        while let Some(batch) = batches.next_item() {
            accepted += match &batch {
                RecordBatch::Rows(rows) => rows.len() as u64,
                RecordBatch::Changes(c) => c.len() as u64,
                RecordBatch::Arrow(ipc) => {
                    let reader =
                        arrow::ipc::reader::StreamReader::try_new(Cursor::new(&ipc.ipc), None)
                            .expect("arrow IPC stream reader");
                    let mut n: u64 = 0;
                    for b in reader {
                        n += b.expect("arrow record batch").num_rows() as u64;
                    }
                    n
                }
            };
        }
        WriteOutcome {
            state: StreamState { cursor: None, opaque: Vec::new() },
            diagnostics: Vec::new(),
            dead_letters: Vec::new(),
            result: WriteResult::Ok(WriteReceipt { accepted }),
        }
    }
}

impl ArrowSink {
    fn configure(cfg: weir_connector_types::Config) -> Self { Self { _cfg: cfg } }
}
