//! WEIR-I-0011 S2: `arrow-sink` as a **wasm destination** — and a **both-ends-wasm**
//! run (slow wasm source → arrow-sink wasm dest), exercising client-streaming
//! `write` over wasm. Proves a wasm dest + that the arrow crate compiles to
//! wasm32-wasip2 (the bulk path's home).

use std::path::Path;

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
fn wasm_slow_to_wasm_arrow_sink() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../wasm-fixtures");

    weir_wasm_testkit::stage(
        &weir_wasm_testkit::WasmPackage {
            fixture_dir: &base.join("slow"),
            wasm_file: "weir_slow_wasm.wasm",
            pkg_name: "weir-slow-pkg",
            capabilities: &[],
        },
        root,
    );
    weir_wasm_testkit::stage(
        &weir_wasm_testkit::WasmPackage {
            fixture_dir: &base.join("arrow-sink"),
            wasm_file: "weir_arrow_sink_wasm.wasm",
            pkg_name: "weir-arrow-sink-pkg",
            capabilities: &[],
        },
        root,
    );

    let store = Store::open(tmp.path().join("weir.db").to_str().unwrap()).expect("open store");
    let cfg = Config {
        json: "{\"sleep_ms\":0,\"rows\":3}".to_string(),
    };
    // Both ends are wasm guests.
    let source = ConnectorHandle::from_wasm_package(
        root,
        "weir-slow-pkg",
        &cfg,
        weir_runtime::HostAllowList::allow_all(),
        &[],
    )
    .expect("load slow wasm");
    let dest = ConnectorHandle::from_wasm_package(
        root,
        "weir-arrow-sink-pkg",
        &cfg,
        weir_runtime::HostAllowList::allow_all(),
        &[],
    )
    .expect("load arrow-sink wasm");

    let out = Engine::new(&store)
        .sync("slow→arrow-wasm", &slow_stream(), &source, &dest)
        .expect("both-ends-wasm sync");
    assert_eq!(out.chunks, 3);
    assert_eq!(
        out.rows_written, 3,
        "wasm arrow-sink counted the 3 streamed rows"
    );
}
