//! # weir-connector
//!
//! The **connector contract** for weir — the single, capability-gated `fidius`
//! interface every connector implements, plus its boundary types.
//! Realizes [`WEIR-A-0014`] / [`WEIR-S-0006`].
//!
//! ## Shape (v1, [`WEIR-A-0029`])
//! - `spec` / `check` / `discover` are **typed** methods (small payloads).
//! - `read` is a **typed server-streaming** method: `ReadContext` →
//!   `Stream<ReadMessage>`, yielding records / checkpoints / logs / dead-letters
//!   lazily with backpressure (the engine commits on checkpoints).
//! - `write` is a **`#[wire(raw)]`** method: a single `Vec<u8>` in/out carrying a
//!   bincode [`envelope`](crate::encode) with Arrow IPC bytes inside. Raw avoids
//!   exploding a byte buffer into a per-element `list<u8>` on the WASM path.
//! - `config` is bound at **construction** (configured instances), not passed on
//!   each call.
//!
//! Capability roles (Source / Destination / ReverseEtl) are advertised in
//! [`ConnectorSpec`]; the authoritative per-method capability bits come from
//! fidius `#[optional]` on the descriptor, which the host queries at dispatch.

// Boundary types, error taxonomy, envelope codec, and DiscoverOutcome live in
// the fidius-free `weir-connector-types` crate so WASM guests can share them
// (the host facade `fidius` is not wasm-buildable).
pub use weir_connector_types::*;

// White-label fidius (fidius how-to: white-label-interface): connector authors
// depend only on `weir-connector` and never import `fidius` directly. The
// `crate = "crate::fidius"` attribute on the interface (and
// `crate = "weir_connector::fidius"` on a connector's `#[plugin_impl]`) resolves
// fidius through this re-export.
pub use fidius;
pub use fidius::{PluginError, plugin_impl};

/// The weir connector contract (v0).
///
/// One connector implements the methods for the role(s) it provides; a pure
/// source leaves `write` unimplemented (its `#[optional]` capability bit stays
/// off), and the host never dispatches writes to it.
#[fidius::plugin_interface(version = 1, buffer = PluginAllocated, crate = "crate::fidius")]
pub trait Connector: Send + Sync {
    /// Static descriptor: config schema, declared roles, supported sync modes.
    fn spec(&self) -> ConnectorSpec;

    /// Validate configuration and connectivity. Moves no data.
    fn check(&self, config: Config) -> CheckResult;

    /// Introspect available streams, schemas, cursors, keys, and partitionability.
    /// (SOURCE capability.)
    #[optional(since = 1)]
    fn discover(&self, config: Config) -> DiscoverOutcome;

    /// Stream records, checkpoints, logs, and dead-letters for one
    /// `(stream, partition)` from a resume point (SOURCE capability). v1 streaming
    /// ([[WEIR-A-0029]]); `config` is bound at construction (configured instance).
    #[optional(since = 1)]
    fn read(&self, ctx: ReadContext) -> crate::fidius::Stream<ReadMessage>;

    /// Write a **client-stream** of batches to the destination (DESTINATION
    /// capability, [[WEIR-A-0029]]): the host produces the batches, the connector
    /// pulls them at its own pace (backpressure), and returns one [`WriteOutcome`].
    /// `config` is bound at construction.
    #[optional(since = 1)]
    fn write(&self, ctx: WriteContext, batches: crate::fidius::Stream<RecordBatch>)
    -> WriteOutcome;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_schema() -> ArrowSchemaIpc {
        // Opaque at the contract layer; real Arrow IPC bytes arrive with the
        // SDK/runtime (WEIR-T-0003/0004). Dummy bytes exercise serde round-trip.
        ArrowSchemaIpc {
            ipc: vec![0xAA, 0xBB, 0xCC],
        }
    }

    fn sample_configured_stream() -> ConfiguredStream {
        ConfiguredStream {
            stream: "users".into(),
            sync_mode: SyncMode::Incremental,
            cursor_field: Some("updated_at".into()),
            primary_key: Some(vec!["id".into()]),
            write_mode: WriteMode::Upsert {
                business_keys: vec!["id".into()],
            },
            mapping: MappingSpec {
                ops: vec![MappingOp::Drop {
                    fields: vec!["secret".into()],
                }],
            },
        }
    }

    /// Every typed boundary value survives a JSON round-trip.
    #[test]
    fn typed_types_json_round_trip() {
        let spec = ConnectorSpec {
            name: "example".into(),
            connector_version: "0.1.0".into(),
            contract_version: 0,
            config_schema: json!({"type": "object"}).to_string(),
            roles: vec![ConnectorRole::Source, ConnectorRole::Destination],
            supported_sync_modes: vec![SyncMode::FullRefresh, SyncMode::Incremental],
        };
        let catalog = Catalog {
            streams: vec![StreamInfo {
                name: "users".into(),
                namespace: None,
                schema: sample_schema(),
                supported_sync_modes: vec![SyncMode::Incremental],
                source_defined_cursor: true,
                default_cursor_field: Some("updated_at".into()),
                source_defined_primary_key: Some(vec!["id".into()]),
                partitioning: PartitionScheme::ByCursorRange {
                    granularity: "P1D".into(),
                },
            }],
        };

        for value_ok in [
            serde_json::to_string(&spec)
                .map(|s| serde_json::from_str::<ConnectorSpec>(&s).unwrap() == spec),
            serde_json::to_string(&catalog)
                .map(|s| serde_json::from_str::<Catalog>(&s).unwrap() == catalog),
        ] {
            assert!(value_ok.unwrap());
        }

        let outcome = DiscoverOutcome::Error(
            ConnectorError::config("missing api_key").with_context("field", "api_key"),
        );
        let s = serde_json::to_string(&outcome).unwrap();
        assert_eq!(
            serde_json::from_str::<DiscoverOutcome>(&s).unwrap(),
            outcome
        );
    }

    /// The v1 read wire round-trips through the bincode codec: the `ReadContext`
    /// (config bound at construction, so not on the context) and every
    /// `ReadMessage` variant the streaming read can yield.
    #[test]
    fn read_wire_round_trip() {
        let ctx = ReadContext {
            stream: sample_configured_stream(),
            partition: Partition {
                id: "p0".into(),
                bounds: Some(json!({"day": "2026-06-17"}).to_string()),
            },
            state: StreamState {
                cursor: Some(json!("2026-06-16").to_string()),
                opaque: vec![1, 2, 3],
            },
        };
        let bytes = encode(&ctx).unwrap();
        assert_eq!(decode::<ReadContext>(&bytes).unwrap(), ctx);

        // Each ReadMessage variant the host decodes off the stream.
        let messages = vec![
            ReadMessage::Records(RecordBatch::Arrow(ArrowIpc { ipc: vec![9, 8, 7] })),
            ReadMessage::Checkpoint(StreamState {
                cursor: Some(json!("2026-06-17").to_string()),
                opaque: vec![],
            }),
            ReadMessage::Log(LogEntry {
                level: LogLevel::Info,
                message: "page 1".into(),
            }),
            ReadMessage::DeadLettered(DeadLetter {
                record: "{\"bad\":true}".into(),
                reason: "rejected".into(),
            }),
            // The error arm ends the stream (its `kind` drives retry in the engine).
            ReadMessage::Fatal(ConnectorError::transient("rate limited")),
        ];
        for msg in messages {
            let bytes = encode(&msg).unwrap();
            assert_eq!(decode::<ReadMessage>(&bytes).unwrap(), msg);
        }
    }

    /// The v1 client-streaming `write` wire round-trips: the `WriteContext` arg,
    /// a `RecordBatch` stream item, and the `WriteOutcome` return.
    #[test]
    fn write_wire_round_trip() {
        let ctx = WriteContext {
            stream: sample_configured_stream(),
        };
        let bytes = encode(&ctx).unwrap();
        assert_eq!(decode::<WriteContext>(&bytes).unwrap(), ctx);

        let batch = RecordBatch::Rows(vec![
            json!({"id": 1}).to_string(),
            json!({"id": 2}).to_string(),
        ]);
        let bytes = encode(&batch).unwrap();
        assert_eq!(decode::<RecordBatch>(&bytes).unwrap(), batch);
        assert_eq!(batch.row_count_hint(), Some(2));

        for outcome in [
            WriteOutcome {
                state: StreamState::default(),
                diagnostics: vec![],
                dead_letters: vec![],
                result: WriteResult::Ok(WriteReceipt { accepted: 2 }),
            },
            WriteOutcome {
                state: StreamState::default(),
                diagnostics: vec![],
                dead_letters: vec![],
                result: WriteResult::Err(ConnectorError::transient("db down")),
            },
        ] {
            let bytes = encode(&outcome).unwrap();
            assert_eq!(decode::<WriteOutcome>(&bytes).unwrap(), outcome);
        }
    }
}
