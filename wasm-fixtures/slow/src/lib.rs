//! Hand-written WASM guest for the `slow` connector (WEIR-I-0011 S2): emits one
//! row per checkpoint, sleeping between, so a run visibly progresses. The
//! `weir_guest_types` block + `Connector` trait are copied verbatim from the
//! reference guest (they ARE the contract — verbatim keeps the WIT hash matching
//! the host); only the `impl` is connector-specific.
#![allow(clippy::all)]

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

pub struct Slow { cfg: weir_connector_types::Config }

#[plugin_impl(Connector, crate = "fidius_guest", config = weir_connector_types::Config)]
impl Connector for Slow {
    fn spec(&self) -> ConnectorSpec {
        ConnectorSpec {
            name: "slow".to_string(),
            connector_version: "0.1.0".to_string(),
            contract_version: 1,
            config_schema: "{\"type\":\"object\",\"properties\":{\"sleep_ms\":{\"type\":\"integer\"},\"rows\":{\"type\":\"integer\"}}}".to_string(),
            roles: vec![ConnectorRole::Source],
            supported_sync_modes: vec![SyncMode::FullRefresh],
        }
    }

    fn check(&self, _config: Config) -> CheckResult {
        CheckResult { success: true, message: None }
    }

    fn discover(&self, _config: Config) -> DiscoverOutcome {
        DiscoverOutcome::Catalog(Catalog {
            streams: vec![StreamInfo {
                name: "slow".to_string(),
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

    /// Emit one row per checkpoint, sleeping between, so a run visibly progresses;
    /// `sleep_ms` is spread across the rows (`sleep_ms:0` ⇒ instant, for tests).
    fn read(&self, ctx: ReadContext) -> fidius_guest::Stream<ReadMessage> {
        let v: serde_json::Value =
            serde_json::from_str(&self.cfg.json).unwrap_or(serde_json::Value::Null);
        let sleep_ms = v.get("sleep_ms").and_then(|x| x.as_u64()).unwrap_or(2000);
        let rows = v.get("rows").and_then(|x| x.as_u64()).unwrap_or(10);
        let prev: u64 = ctx
            .state
            .cursor
            .as_deref()
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);
        let per = if rows > 0 { sleep_ms / rows } else { 0 };

        // Bulk mode (`batch:true`): emit all rows in ONE Records + one Checkpoint —
        // isolates the streaming-surface throughput (no per-row checkpoint commits).
        // Used by the throughput benchmark (WEIR-T-0045).
        if v.get("batch").and_then(|b| b.as_bool()) == Some(true) {
            let batch: Vec<String> = (1..=rows).map(|i| format!("{{\"n\":{}}}", prev + i)).collect();
            return fidius_guest::Stream::from_iter(vec![
                ReadMessage::Records(RecordBatch::Rows(batch)),
                ReadMessage::Checkpoint(StreamState {
                    cursor: Some((prev + rows).to_string()),
                    opaque: Vec::new(),
                }),
            ]);
        }

        let log = std::iter::once(ReadMessage::Log(LogEntry {
            level: LogLevel::Info,
            message: format!(
                "slow source: emitting {rows} rows ({per}ms each), cursor {prev} → {}",
                prev + rows
            ),
        }));
        let chunks = (1..=rows).flat_map(move |i| {
            if per > 0 {
                std::thread::sleep(std::time::Duration::from_millis(per));
            }
            [
                ReadMessage::Records(RecordBatch::Rows(vec![format!("{{\"n\":{}}}", prev + i)])),
                ReadMessage::Checkpoint(StreamState {
                    cursor: Some((prev + i).to_string()),
                    opaque: Vec::new(),
                }),
            ]
        });
        fidius_guest::Stream::from_iter(log.chain(chunks))
    }

    fn write(&self, _ctx: WriteContext, mut batches: fidius_guest::Stream<RecordBatch>) -> WriteOutcome {
        while batches.next_item().is_some() {}
        WriteOutcome {
            state: StreamState { cursor: None, opaque: Vec::new() },
            diagnostics: Vec::new(),
            dead_letters: Vec::new(),
            result: WriteResult::Err(ConnectorError::fatal("slow is a source, not a destination")),
        }
    }
}

impl Slow {
    fn configure(cfg: weir_connector_types::Config) -> Self { Self { cfg } }
}
