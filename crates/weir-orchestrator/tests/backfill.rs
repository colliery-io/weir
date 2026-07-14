//! WEIR-T-0010: a backfill re-syncs under an isolated checkpoint key, so the
//! live incremental cursor is never corrupted.

mod common;
use common::wasm_ref;

use std::sync::Arc;

use weir_connector::{Config, ConfiguredStream, MappingSpec, SyncMode, WriteMode};
use weir_engine::Store;
use weir_orchestrator::{InProcessExecutor, Relay, WorkSpec, Worker, WorkerConfig};

fn spec(emit_cursor: &str) -> WorkSpec {
    WorkSpec {
        connection: "bf-conn".to_string(),
        tenant: "default".to_string(),
        stream: ConfiguredStream {
            stream: "posts".to_string(),
            sync_mode: SyncMode::Incremental,
            cursor_field: Some("updated_at".to_string()),
            primary_key: None,
            write_mode: WriteMode::Append,
            mapping: MappingSpec::default(),
        },
        // Faulty echoes `emit_cursor` into next_state so the checkpoint advances.
        source: wasm_ref("Faulty"),
        dest: wasm_ref("ArrowSink"),
        source_config: Config {
            json: format!("{{\"emit_cursor\":\"{emit_cursor}\"}}"),
        },
        dest_config: Config {
            json: format!("{{\"emit_cursor\":\"{emit_cursor}\"}}"),
        },
        state_key: None,
        seed_cursor: None,
        partition: None,
        execution_mode: Default::default(),
    }
}

async fn drain(relay: &Relay, store: &Arc<Store>) {
    Worker::new(
        relay.clone(),
        InProcessExecutor::new(Arc::clone(store), relay.clone()),
        WorkerConfig::default(),
    )
    .run_until_idle()
    .await
    .unwrap();
}

#[tokio::test]
async fn backfill_does_not_corrupt_live_cursor() {
    let tmp = tempfile::TempDir::new().unwrap();
    let store = Arc::new(Store::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let relay = Relay::new(Arc::clone(&store)).unwrap();

    // A normal run advances the live cursor to LIVE-9.
    relay.plan(&spec("LIVE-9")).unwrap();
    drain(&relay, &store).await;
    assert_eq!(
        store.cursor("bf-conn", "posts").unwrap().as_deref(),
        Some("LIVE-9")
    );

    // A backfill from an earlier point runs under its own key (emits BF-1).
    relay.plan_backfill(&spec("BF-1"), "OLD-1").unwrap();
    drain(&relay, &store).await;

    // The live cursor is UNTOUCHED; the backfill advanced only its own key.
    assert_eq!(
        store.cursor("bf-conn", "posts").unwrap().as_deref(),
        Some("LIVE-9"),
        "live incremental cursor must survive the backfill"
    );
    assert_eq!(
        store
            .cursor("bf-conn", "backfill:posts:OLD-1")
            .unwrap()
            .as_deref(),
        Some("BF-1"),
        "backfill checkpointed under its isolated key"
    );
}
