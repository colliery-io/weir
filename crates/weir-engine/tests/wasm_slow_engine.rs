//! WEIR-I-0011 S2: a **hand-written** connector (`slow`) running as a **wasm
//! guest** through the Sync Engine — proving the migration pattern (guest crate
//! built + staged via the shared testkit, loaded via `from_wasm_package`, driven
//! by the engine's off-tokio read loop). slow (wasm source) → arrow-sink (native
//! dest); slow needs no host capabilities.

use std::path::Path;

use weir_connector::{
    CompareOp, ComputeExpr, Config, ConfiguredStream, MappingOp, MappingSpec, SyncMode, WriteMode,
};
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
fn wasm_slow_source_through_engine() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();

    // Build + stage the slow wasm guest (cached) — no capabilities needed.
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
        json: "{\"sleep_ms\":0,\"rows\":3}".to_string(),
    };
    // slow as a WASM guest; arrow-sink stays native.
    let source = ConnectorHandle::from_wasm_package(
        root,
        "weir-slow-pkg",
        &cfg,
        weir_runtime::HostAllowList::allow_all(),
        &[],
    )
    .expect("load slow wasm");
    let dest = weir_wasm_testkit::load("ArrowSink", &cfg).expect("arrow sink");

    let out = Engine::new(&store)
        .sync("slow-wasm", &slow_stream(), &source, &dest)
        .expect("sync slow wasm → arrow sink");

    // One row per checkpoint → 3 checkpoints, 3 rows; cursor advanced to "3".
    assert_eq!(out.chunks, 3, "one checkpoint per row");
    assert_eq!(out.rows_written, 3);
    assert_eq!(out.final_state.cursor.as_deref(), Some("3"));
}

/// WEIR-T-0052: the in-flight mapping stage applied **through the engine** — slow
/// emits `{"n":1..5}` in one batch; a `MappingSpec` filters `n > 3` + renames +
/// computes, and the destination receives only the mapped/kept records.
#[test]
fn wasm_mapping_filter_through_engine() {
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

    let store = Store::open(tmp.path().join("map.db").to_str().unwrap()).expect("open store");
    // Bulk mode: 5 rows in one Records batch → one checkpoint.
    let cfg = Config {
        json: "{\"sleep_ms\":0,\"rows\":5,\"batch\":true}".to_string(),
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

    let stream = ConfiguredStream {
        stream: "slow".to_string(),
        sync_mode: SyncMode::FullRefresh,
        cursor_field: None,
        primary_key: None,
        write_mode: WriteMode::Append,
        mapping: MappingSpec {
            ops: vec![
                MappingOp::Filter {
                    field: "n".to_string(),
                    op: CompareOp::Gt,
                    value: "3".to_string(),
                },
                MappingOp::Rename {
                    from: "n".to_string(),
                    to: "num".to_string(),
                },
                MappingOp::Compute {
                    field: "label".to_string(),
                    value: ComputeExpr::Const("kept".to_string()),
                },
            ],
        },
    };

    let out = Engine::new(&store)
        .sync("map-wasm", &stream, &source, &dest)
        .expect("sync with mapping");

    // 5 rows in; filter n > 3 keeps {4, 5} → 2 written (the engine applied the map).
    assert_eq!(out.rows_written, 2, "filter n>3 keeps 2 of 5 rows");
}
