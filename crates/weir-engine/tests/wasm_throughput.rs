//! WEIR-I-0011 S5: a streaming-throughput benchmark for the WASM connector path —
//! a bulk batch (slow `batch:true`) flows wasm source → engine → wasm sink, so it
//! exercises both wasm boundaries (read pull + client-streamed write) without
//! per-row checkpoint commits. `#[ignore]` (a benchmark, not a gate). Run with:
//! `cargo test -p weir-engine --test wasm_throughput -- --ignored --nocapture`.

use std::path::Path;
use std::time::Instant;

use weir_connector::{Config, ConfiguredStream, MappingSpec, SyncMode, WriteMode};
use weir_engine::{Engine, Store};
use weir_runtime::ConnectorHandle;

fn slow_stream() -> ConfiguredStream {
    ConfiguredStream {
        stream: "slow".to_string(),
        sync_mode: SyncMode::FullRefresh,
        cursor_field: None,
        primary_key: None,
        write_mode: WriteMode::Append,
        mapping: MappingSpec::default(),
    }
}

#[test]
#[ignore = "benchmark — run with --ignored --nocapture"]
fn wasm_streaming_throughput() {
    const ROWS: u64 = 500_000;

    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../wasm-fixtures/slow");
    weir_wasm_testkit::stage(
        &weir_wasm_testkit::WasmPackage {
            fixture_dir: &fixture,
            wasm_file: "weir_slow_wasm.wasm",
            pkg_name: "weir-slow-pkg",
            capabilities: &[],
        },
        root,
    );

    let store = Store::open(tmp.path().join("weir.db").to_str().unwrap()).expect("open store");
    let cfg = Config {
        json: format!("{{\"sleep_ms\":0,\"rows\":{ROWS},\"batch\":true}}"),
    };
    let source = ConnectorHandle::from_wasm_package(
        root,
        "weir-slow-pkg",
        &cfg,
        weir_runtime::HostAllowList::allow_all(),
        &[],
    )
    .expect("load slow wasm");
    let dest = weir_wasm_testkit::load("ArrowSink", &cfg).expect("arrow sink");

    let engine = Engine::new(&store);
    let start = Instant::now();
    let out = engine
        .sync("bench", &slow_stream(), &source, &dest)
        .expect("bench sync");
    let dt = start.elapsed();

    assert_eq!(out.rows_written, ROWS, "all rows flowed through");

    // Each row is `{"n":NNN}` ≈ 9–13 bytes; estimate payload bytes for an MB/s figure.
    let bytes: u64 = (1..=ROWS)
        .map(|i| format!("{{\"n\":{i}}}").len() as u64)
        .sum();
    let secs = dt.as_secs_f64();
    eprintln!("\n==== WASM streaming throughput (WEIR-T-0045) ====");
    eprintln!("  rows        : {ROWS}");
    eprintln!("  payload     : {:.2} MB", bytes as f64 / 1.0e6);
    eprintln!("  wall time   : {dt:?}");
    eprintln!(
        "  throughput  : {:.0} rows/s  |  {:.1} MB/s",
        ROWS as f64 / secs,
        bytes as f64 / secs / 1.0e6
    );
    eprintln!("  (wasm source → engine → wasm arrow-sink; both boundaries, one checkpoint)\n");
}
