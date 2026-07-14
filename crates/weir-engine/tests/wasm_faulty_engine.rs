//! WEIR-I-0011 S2: the `faulty` connector as a **wasm guest** through the engine —
//! proving the error + dead-letter channels round-trip over wasm (not just the
//! happy path). faulty (wasm) → arrow-sink (native).

use std::path::Path;

use weir_connector::{Config, ConfiguredStream, MappingSpec, SyncMode, WriteMode};
use weir_engine::{Engine, EngineError, Store};
use weir_runtime::ConnectorHandle;

fn faulty_stream() -> ConfiguredStream {
    ConfiguredStream {
        stream: "faulty".to_string(),
        sync_mode: SyncMode::FullRefresh,
        cursor_field: None,
        primary_key: None,
        write_mode: WriteMode::Append,
        mapping: MappingSpec::default(),
    }
}

fn stage_faulty(root: &Path) {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../wasm-fixtures/faulty");
    weir_wasm_testkit::stage(
        &weir_wasm_testkit::WasmPackage {
            fixture_dir: &fixture,
            wasm_file: "weir_faulty_wasm.wasm",
            pkg_name: "weir-faulty-pkg",
            capabilities: &[],
        },
        root,
    );
}

#[test]
fn wasm_faulty_dead_letters_through_engine() {
    let tmp = tempfile::TempDir::new().unwrap();
    stage_faulty(tmp.path());
    let store = Store::open(tmp.path().join("weir.db").to_str().unwrap()).expect("open store");
    let cfg = Config {
        json: "{\"dead_letter\":true}".to_string(),
    };
    let source = ConnectorHandle::from_wasm_package(
        tmp.path(),
        "weir-faulty-pkg",
        &cfg,
        weir_runtime::HostAllowList::allow_all(),
        &[],
    )
    .expect("load faulty");
    let dest = weir_wasm_testkit::load("ArrowSink", &cfg).expect("arrow sink");

    let out = Engine::new(&store)
        .sync("faulty-dl", &faulty_stream(), &source, &dest)
        .expect("dead-letter run still succeeds");
    assert_eq!(out.dead_lettered, 1, "one record dead-lettered");
    assert_eq!(out.rows_written, 1, "the good record still written");
}

#[test]
fn wasm_faulty_fatal_through_engine() {
    let tmp = tempfile::TempDir::new().unwrap();
    stage_faulty(tmp.path());
    let store = Store::open(tmp.path().join("weir.db").to_str().unwrap()).expect("open store");
    let cfg = Config {
        json: "{\"fail\":\"fatal\",\"token\":\"wasm-fatal\"}".to_string(),
    };
    let source = ConnectorHandle::from_wasm_package(
        tmp.path(),
        "weir-faulty-pkg",
        &cfg,
        weir_runtime::HostAllowList::allow_all(),
        &[],
    )
    .expect("load faulty");
    let dest = weir_wasm_testkit::load("ArrowSink", &cfg).expect("arrow sink");

    let res = Engine::new(&store).sync("faulty-fatal", &faulty_stream(), &source, &dest);
    assert!(
        matches!(res, Err(EngineError::Connector(_))),
        "fatal connector error surfaces as EngineError::Connector, got {res:?}"
    );
}
