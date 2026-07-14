//! The Sync Engine slice end-to-end (WEIR-T-0005): drive `echo` source → engine
//! → `echo` destination, asserting the chunk loop runs and the checkpoint +
//! outbox commit (transactionally) land in the diesel-dualdb store.

use weir_connector::{Config, ConfiguredStream, MappingSpec, SyncMode, WriteMode};
use weir_engine::{Engine, Store, SyncOptions, SyncProgress};

fn configured_stream() -> ConfiguredStream {
    ConfiguredStream {
        stream: "echo".to_string(),
        sync_mode: SyncMode::FullRefresh,
        cursor_field: None,
        primary_key: None,
        write_mode: WriteMode::Append,
        mapping: MappingSpec::default(),
    }
}

#[test]
fn engine_syncs_echo_with_transactional_checkpoint() {
    // Temp file DB so every pooled connection sees the same SQLite database.
    let tmp = tempfile::TempDir::new().unwrap();
    let db = tmp.path().join("weir.db");
    let store = Store::open(db.to_str().unwrap()).expect("open store");

    let engine = Engine::new(&store);
    let cfg = Config {
        json: "{}".to_string(),
    };
    let source = weir_wasm_testkit::load("Echo", &cfg).expect("source");
    let dest = weir_wasm_testkit::load("Echo", &cfg).expect("dest");

    // Echo returns one chunk (has_more = false) → one commit.
    let out = engine
        .sync("conn-1", &configured_stream(), &source, &dest)
        .expect("sync");
    assert_eq!(out.chunks, 1);
    assert_eq!(
        store.outbox_count("conn-1").unwrap(),
        1,
        "checkpoint + outbox row committed atomically"
    );

    // State persists across runs; a second sync commits another chunk.
    let out2 = engine
        .sync("conn-1", &configured_stream(), &source, &dest)
        .expect("second sync");
    assert_eq!(out2.chunks, 1);
    assert_eq!(store.outbox_count("conn-1").unwrap(), 2);
    assert_eq!(out2.final_state, out.final_state);
}

/// WEIR-T-0037: the engine reports cumulative progress on each committed
/// checkpoint, so the orchestrator can update the in-flight unit (live feed).
#[test]
fn sync_reports_progress_per_checkpoint() {
    let tmp = tempfile::TempDir::new().unwrap();
    let store = Store::open(tmp.path().join("weir.db").to_str().unwrap()).expect("open store");
    let engine = Engine::new(&store);

    // Slow (sleep_ms:0) emits one row per checkpoint → 3 checkpoints, no delay.
    let cfg = Config {
        json: "{\"sleep_ms\":0,\"rows\":3}".to_string(),
    };
    let source = weir_wasm_testkit::load("Slow", &cfg).expect("slow source");
    let dest = weir_wasm_testkit::load("ArrowSink", &cfg).expect("arrow sink");
    let stream = ConfiguredStream {
        stream: "slow".to_string(),
        sync_mode: SyncMode::FullRefresh,
        cursor_field: None,
        primary_key: None,
        write_mode: WriteMode::Append,
        mapping: MappingSpec::default(),
    };

    let mut events: Vec<SyncProgress> = Vec::new();
    let out = engine
        .sync_with(
            "prog",
            &stream,
            &source,
            &dest,
            &SyncOptions::default(),
            &mut |p| events.push(p),
        )
        .expect("sync");

    assert_eq!(out.chunks, 3, "one checkpoint per row");
    assert_eq!(events.len(), 3, "a progress event per committed checkpoint");
    // Cumulative + monotonic: rows climb 1, 2, 3.
    assert_eq!(events[0].rows_written, 1);
    assert_eq!(events[2].rows_written, 3);
    assert_eq!(events[2].chunks, 3);
}

#[test]
fn schema_enforcement_dead_letters_violations() {
    // [[WEIR-T-0119]]: with a stored schema, records that don't coerce to it dead-letter (not written).
    use weir_connector::{Config, ConfiguredStream, MappingSpec, SyncMode, WriteMode};
    let tmp = tempfile::TempDir::new().unwrap();
    let store = Store::open(tmp.path().join("weir.db").to_str().unwrap()).expect("store");
    // Seed a schema where `n` is boolean — Slow's integer `n` will violate it.
    store
        .capture_schema("conn-e", "se", &["{\"n\": true}".to_string()])
        .expect("seed schema");

    let engine = Engine::new(&store);
    let slow = weir_wasm_testkit::load(
        "Slow",
        &Config {
            json: "{\"rows\":3,\"batch\":true,\"sleep_ms\":0}".to_string(),
        },
    )
    .expect("slow");
    let arrow = weir_wasm_testkit::load(
        "ArrowSink",
        &Config {
            json: "{}".to_string(),
        },
    )
    .expect("arrow");
    let st = ConfiguredStream {
        stream: "se".to_string(),
        sync_mode: SyncMode::FullRefresh,
        cursor_field: None,
        primary_key: None,
        write_mode: WriteMode::Append,
        mapping: MappingSpec::default(),
    };

    let out = engine.sync("conn-e", &st, &slow, &arrow).expect("sync");
    assert_eq!(
        out.rows_written, 0,
        "all rows violate the boolean schema → none written"
    );
    assert_eq!(out.dead_lettered, 3, "3 violations dead-lettered");
}

#[test]
fn breaking_drift_flags_broken_and_blocks() {
    // [[WEIR-T-0120]]: a type change vs the stored schema is flagged breaking + the records dead-letter
    // (enforcement blocks them from the destination).
    use weir_connector::{Config, ConfiguredStream, MappingSpec, SyncMode, WriteMode};
    let tmp = tempfile::TempDir::new().unwrap();
    let store = Store::open(tmp.path().join("weir.db").to_str().unwrap()).expect("store");
    // Stored schema says `n` is boolean; Slow will send integers → breaking type drift.
    store
        .capture_schema("conn-d", "sd", &["{\"n\": true}".to_string()])
        .expect("seed");

    let engine = Engine::new(&store);
    let slow = weir_wasm_testkit::load(
        "Slow",
        &Config {
            json: "{\"rows\":3,\"batch\":true,\"sleep_ms\":0}".to_string(),
        },
    )
    .expect("slow");
    let arrow = weir_wasm_testkit::load(
        "ArrowSink",
        &Config {
            json: "{}".to_string(),
        },
    )
    .expect("arrow");
    let st = ConfiguredStream {
        stream: "sd".to_string(),
        sync_mode: SyncMode::FullRefresh,
        cursor_field: None,
        primary_key: None,
        write_mode: WriteMode::Append,
        mapping: MappingSpec::default(),
    };

    let out = engine.sync("conn-d", &st, &slow, &arrow).expect("sync");
    assert_eq!(
        out.dead_lettered, 3,
        "breaking-typed records dead-letter (blocked)"
    );
    let broken = store.schema_broken("conn-d", "sd").expect("query broken");
    assert!(broken.is_some(), "breaking drift flagged");
    assert!(broken.unwrap().contains('n'), "reason names the field");
}

// WEIR-I-0035 F1.5: resident-loop behaviour through the real engine + a real wasm source.
// A resident source whose stream *ends* is abnormal (end-of-stream is not expected while resident)
// → `run_resident` returns `Err(ResidentStreamEnded)` so the worker requeues (F1.3), and the
// checkpoint/outbox from the drained chunk is still committed so the restart resumes cleanly.
#[test]
fn run_resident_errs_on_stream_end_leaving_checkpoint_intact() {
    let tmp = tempfile::TempDir::new().unwrap();
    let db = tmp.path().join("weir.db");
    let store = Store::open(db.to_str().unwrap()).expect("open store");

    let engine = Engine::new(&store);
    let cfg = Config {
        json: "{}".to_string(),
    };
    let source = weir_wasm_testkit::load("Echo", &cfg).expect("source");
    let dest = weir_wasm_testkit::load("Echo", &cfg).expect("dest");

    // Named `_handle` (not `_`) so drop-to-cancel does NOT fire early — the stream ends on its own.
    let (_handle, token) = weir_engine::stop_channel();
    let out = engine.run_resident(
        "conn-res",
        &configured_stream(),
        &source,
        &dest,
        &SyncOptions::default(),
        &mut |_| {},
        token,
        None, // no cadence pacing for this stream-end test
    );
    assert!(
        matches!(out, Err(weir_engine::EngineError::ResidentStreamEnded)),
        "resident stream-end → Err so the worker requeues; got {out:?}"
    );
    assert_eq!(
        store.outbox_count("conn-res").unwrap(),
        1,
        "the drained chunk's checkpoint + outbox committed → resume point intact"
    );
}
