//! Hand-written WASM guest for the `faulty` connector (WEIR-I-0011 S2): fails on
//! demand via the contract's error + dead-letter channels. The `weir_guest_types`
//! block + `Connector` trait are verbatim (they ARE the contract — keeps the WIT
//! hash matching the host); only the `impl` is connector-specific.
#![allow(clippy::all)]

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

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

/// Per-token call counters, so concurrent tests don't share state.
static COUNTERS: LazyLock<Mutex<HashMap<String, u64>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub struct Faulty { cfg: weir_connector_types::Config }

/// Evaluate a config into (dead-letters, operation result).
fn evaluate(config: &weir_connector_types::Config) -> (Vec<DeadLetter>, Result<(), ConnectorError>) {
    let v: serde_json::Value =
        serde_json::from_str(&config.json).unwrap_or(serde_json::Value::Null);

    let dead = if v.get("dead_letter").and_then(|d| d.as_bool()) == Some(true) {
        vec![DeadLetter {
            record: "{\"bad\":true}".to_string(),
            reason: "simulated record-level rejection".to_string(),
        }]
    } else {
        Vec::new()
    };

    let kind = match v.get("fail") {
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        Some(serde_json::Value::Bool(true)) => Some("fatal".to_string()),
        _ => None,
    };
    let Some(kind) = kind else {
        return (dead, Ok(()));
    };

    let Some(token) = v.get("token").and_then(|t| t.as_str()) else {
        return (
            dead,
            Err(ConnectorError {
                kind: ErrorKind::Config,
                message: "faulty requires a `token` when `fail` is set".to_string(),
                retryable: false,
                context: Vec::new(),
            }),
        );
    };

    let count = {
        let mut map = COUNTERS.lock().expect("counters");
        let n = map.entry(token.to_string()).or_insert(0);
        *n += 1;
        *n
    };
    let should_fail = match v.get("until").and_then(|u| u.as_u64()) {
        Some(until) => count <= until,
        None => true,
    };
    if should_fail {
        let err = if kind == "transient" {
            ConnectorError::transient("simulated transient failure")
        } else {
            ConnectorError::fatal("simulated fatal failure")
        };
        return (dead, Err(err));
    }
    (dead, Ok(()))
}

#[plugin_impl(Connector, crate = "fidius_guest", config = weir_connector_types::Config)]
impl Connector for Faulty {
    fn spec(&self) -> ConnectorSpec {
        ConnectorSpec {
            name: "faulty".to_string(),
            connector_version: "0.1.0".to_string(),
            contract_version: 1,
            config_schema: "{\"type\":\"object\",\"properties\":{\"fail\":{\"type\":\"string\"},\"token\":{\"type\":\"string\"},\"until\":{\"type\":\"integer\"},\"dead_letter\":{\"type\":\"boolean\"}}}".to_string(),
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
                name: "faulty".to_string(),
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

    fn read(&self, _ctx: ReadContext) -> fidius_guest::Stream<ReadMessage> {
        let (dead_letters, outcome) = evaluate(&self.cfg);
        let cursor = serde_json::from_str::<serde_json::Value>(&self.cfg.json)
            .ok()
            .and_then(|v| v.get("emit_cursor").and_then(|c| c.as_str()).map(str::to_string));
        let mut msgs: Vec<ReadMessage> = dead_letters
            .into_iter()
            .map(ReadMessage::DeadLettered)
            .collect();
        match outcome {
            Ok(()) => {
                msgs.push(ReadMessage::Records(RecordBatch::Rows(vec![
                    "{\"faulty\":true}".to_string(),
                ])));
                msgs.push(ReadMessage::Checkpoint(StreamState { cursor, opaque: Vec::new() }));
            }
            Err(e) => msgs.push(ReadMessage::Fatal(e)),
        }
        fidius_guest::Stream::from_iter(msgs)
    }

    fn write(&self, _ctx: WriteContext, mut batches: fidius_guest::Stream<RecordBatch>) -> WriteOutcome {
        while batches.next_item().is_some() {}
        let (dead_letters, outcome) = evaluate(&self.cfg);
        let result = match outcome {
            Ok(()) => WriteResult::Ok(WriteReceipt { accepted: 0 }),
            Err(e) => WriteResult::Err(e),
        };
        WriteOutcome {
            state: StreamState { cursor: None, opaque: Vec::new() },
            diagnostics: Vec::new(),
            dead_letters,
            result,
        }
    }
}

impl Faulty {
    fn configure(cfg: weir_connector_types::Config) -> Self { Self { cfg } }
}
