//! WEIR-T-0006 — the walking-skeleton **end-to-end** round-trip that ratifies
//! [[WEIR-A-0014]]: a source → Sync Engine → real Arrow destination, with the
//! source run over **both** execution primitives (native dylib + WASM component).

use std::path::Path;
use std::process::Command;

// Link both reference connectors so their inventory registrations run.

use weir_connector::{Config, ConfiguredStream, MappingSpec, SyncMode, WriteMode};
use weir_engine::{Engine, Store};
use weir_runtime::ConnectorHandle;

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

fn build_and_stage_wasm(root: &Path) {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../wasm-fixtures/echo");
    let status = Command::new("cargo")
        .args(["build", "--target", "wasm32-wasip2", "--release"])
        .current_dir(&fixture)
        .status()
        .expect("cargo build wasm echo");
    assert!(status.success(), "wasm echo build failed");
    let art = fixture.join("target/wasm32-wasip2/release/weir_echo_wasm.wasm");
    let bytes = std::fs::read(&art).unwrap();
    let dir = root.join("weir-echo-pkg");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("package.toml"),
        r#"
[package]
name = "weir-echo-pkg"
version = "0.1.0"
interface = "connector"
interface_version = 1
runtime = "wasm"

[metadata]
category = "test"

[wasm]
component = "weir_echo_wasm.wasm"
"#,
    )
    .unwrap();
    std::fs::write(dir.join("weir_echo_wasm.wasm"), bytes).unwrap();
}

/// Native echo source → engine → native Arrow destination.
#[test]
fn e2e_native_source_to_arrow_destination() {
    let tmp = tempfile::TempDir::new().unwrap();
    let store = Store::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let cfg = Config {
        json: "{}".to_string(),
    };
    let source = weir_wasm_testkit::load("Echo", &cfg).unwrap();
    let dest = weir_wasm_testkit::load("ArrowSink", &cfg).unwrap();

    let out = engine
        .sync("c-native", &configured_stream(), &source, &dest)
        .expect("sync");
    assert_eq!(out.chunks, 1);
    assert_eq!(
        out.rows_written, 1,
        "row delivered through engine to the Arrow sink"
    );
    assert_eq!(store.outbox_count("c-native").unwrap(), 1);
}

/// WASM echo source → engine → native Arrow destination. Same contract, the
/// source running as a sandboxed WASM component (WEIR-A-0002) — the proof that
/// the whole pipeline is execution-primitive-agnostic.
#[test]
fn e2e_wasm_source_to_arrow_destination() {
    // Keep the package search dir and the DB file in separate temp dirs so the
    // SQLite file doesn't sit in the PluginHost discovery path.
    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage_wasm(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();

    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let cfg = Config {
        json: "{}".to_string(),
    };
    let source = ConnectorHandle::from_wasm_package(
        pkg_root.path(),
        "weir-echo-pkg",
        &cfg,
        weir_runtime::HostAllowList::allow_all(),
        &[],
    )
    .expect("load wasm echo source");
    let dest = weir_wasm_testkit::load("ArrowSink", &cfg).unwrap();

    let out = engine
        .sync("c-wasm", &configured_stream(), &source, &dest)
        .expect("sync over wasm source");
    assert_eq!(out.chunks, 1);
    assert_eq!(
        out.rows_written, 1,
        "wasm source row delivered through engine to the Arrow sink"
    );
    assert_eq!(store.outbox_count("c-wasm").unwrap(), 1);
}
