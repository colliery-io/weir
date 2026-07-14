//! [[WEIR-I-0035]] F1.9: the unified **resident** connector (`wasm-fixtures/resident`),
//! driven through the real wasm guest via the Sync Engine.
//!
//! - `mode:poll` — a real **warm poll**: each cycle reads a *bounded batch* (`rows_per_poll`
//!   rows, a read-to-end of the current dataset) + a `Checkpoint`; the HOST sleeps the
//!   declared cadence **between polls** (per checkpoint). Proven by: `rows_written ==
//!   chunks * rows_per_poll` (each poll is a bounded batch, NOT an unbounded per-row
//!   stream) AND the poll count is cadence-bounded over the window (NOT thousands).
//! - `mode:tail` — an event-reader: one record per config-supplied arrival; K → K, 0 → 0.

use std::path::Path;
use std::time::Duration;

use weir_connector::{Config, ConfiguredStream, MappingSpec, SyncMode, WriteMode};
use weir_engine::{Engine, Store, SyncOptions, stop_channel};
use weir_runtime::ConnectorHandle;

fn src_stream(name: &str) -> ConfiguredStream {
    ConfiguredStream {
        stream: name.to_string(),
        sync_mode: SyncMode::FullRefresh,
        cursor_field: None,
        primary_key: None,
        write_mode: WriteMode::Append,
        mapping: MappingSpec::default(),
    }
}

/// Stage the unified `resident` fixture and load it configured with `cfg_json`.
fn load_resident(root: &Path, cfg_json: &str) -> ConnectorHandle {
    weir_wasm_testkit::stage(
        &weir_wasm_testkit::WasmPackage {
            fixture_dir: &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../wasm-fixtures/resident"),
            wasm_file: "weir_resident_wasm.wasm",
            pkg_name: "weir-resident-pkg",
            capabilities: &[],
        },
        root,
    );
    ConnectorHandle::from_wasm_package(
        root,
        "weir-resident-pkg",
        &Config {
            json: cfg_json.to_string(),
        },
        weir_runtime::HostAllowList::allow_all(),
        &[],
    )
    .expect("load resident wasm")
}

/// Goal 4 (poll): a resident poll reads a **bounded batch per cadence cycle** — not an
/// unbounded per-row stream. cadence 20ms over ~250ms ⇒ ~12 polls; each poll a bounded
/// batch of `rows_per_poll` rows ⇒ `rows_written == chunks * rows_per_poll`.
#[test]
fn resident_poll_reads_bounded_batch_per_cadence_cycle() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    let source = load_resident(root, r#"{"mode":"poll","rows_per_poll":3}"#);
    let dest =
        weir_wasm_testkit::load("ArrowSink", &Config { json: "{}".into() }).expect("arrow sink");
    let store = Store::open(tmp.path().join("weir.db").to_str().unwrap()).expect("open store");

    // Stop after ~250ms; host paces one poll per 20ms cadence.
    let (handle, token) = stop_channel();
    let jh = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(250));
        drop(handle); // drop-to-cancel
    });
    let out = Engine::new(&store)
        .run_resident(
            "resident-poll",
            &src_stream("resident"),
            &source,
            &dest,
            &SyncOptions::default(),
            &mut |_| {},
            token,
            Some(Duration::from_millis(20)), // declared cadence (floor)
        )
        .expect("resident poll returns Ok on clean stop");
    jh.join().unwrap();

    // Bounded batch per poll: every committed checkpoint carried exactly rows_per_poll rows.
    assert_eq!(
        out.rows_written,
        out.chunks * 3,
        "each poll must be a bounded batch of 3 (rows={}, polls={})",
        out.rows_written,
        out.chunks
    );
    // Cadence-bounded: ~12 polls over 250ms at 20ms — NOT an unbounded stream (which would
    // be thousands). Tolerant range absorbs scheduler jitter.
    assert!(
        (3..=40).contains(&out.chunks),
        "expected ~12 cadence-paced polls over 250ms@20ms, got {} (unbounded stream would be 1000s)",
        out.chunks
    );
}

/// Goal 4 (tail): an event-reader emits per config-supplied arrival — K arrivals → K rows,
/// 0 → 0 (arrival-driven, not clock-driven).
#[test]
fn resident_tail_emits_per_arrival() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    let store = Store::open(tmp.path().join("weir.db").to_str().unwrap()).expect("open store");

    let src3 = load_resident(root, r#"{"mode":"tail","events":["a","b","c"]}"#);
    let dest3 = weir_wasm_testkit::load("ArrowSink", &Config { json: "{}".into() }).expect("sink");
    let out3 = Engine::new(&store)
        .sync("ev3", &src_stream("resident"), &src3, &dest3)
        .expect("tail 3 arrivals");
    assert_eq!(out3.rows_written, 3, "3 arrivals → 3 records");

    let src0 = load_resident(root, r#"{"mode":"tail","events":[]}"#);
    let dest0 = weir_wasm_testkit::load("ArrowSink", &Config { json: "{}".into() }).expect("sink");
    let out0 = Engine::new(&store)
        .sync("ev0", &src_stream("resident"), &src0, &dest0)
        .expect("tail 0 arrivals");
    assert_eq!(out0.rows_written, 0, "0 arrivals → 0 records");
}
