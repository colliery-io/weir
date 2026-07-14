//! The WEIR-A-0002 proof: the **same contract** runs over **both** execution
//! primitives. Loads the in-process native `echo` and the WASM-component `echo`
//! through the one `ConnectorHandle` seam and asserts identical results.

use std::path::Path;
use std::process::Command;

// Force-link the native connector so its inventory registration runs.

use futures::StreamExt;
use weir_connector::{
    Config, ConfiguredStream, MappingSpec, Partition, ReadContext, ReadMessage, RecordBatch,
    StreamState, SyncMode, WriteContext, WriteMode,
};
use weir_runtime::ConnectorHandle;

/// Build the WASM echo guest and stage it as a fidius package under `root`.
fn build_and_stage_wasm(root: &Path) {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../wasm-fixtures/echo");
    let status = Command::new("cargo")
        .args(["build", "--target", "wasm32-wasip2", "--release"])
        .current_dir(&fixture)
        .status()
        .expect("run cargo build --target wasm32-wasip2 for the wasm echo guest");
    assert!(status.success(), "wasm echo guest build failed");

    let art = fixture.join("target/wasm32-wasip2/release/weir_echo_wasm.wasm");
    let bytes = std::fs::read(&art).unwrap_or_else(|e| panic!("read {}: {e}", art.display()));

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

#[tokio::test]
async fn native_and_wasm_produce_identical_results() {
    // `config` is bound at construction (configured instance, WEIR-A-0029).
    let cfg = Config {
        json: "{}".to_string(),
    };
    let native = weir_wasm_testkit::load("Echo", &cfg).expect("load native echo");

    let tmp = tempfile::TempDir::new().unwrap();
    build_and_stage_wasm(tmp.path());
    let wasm = ConnectorHandle::from_wasm_package(
        tmp.path(),
        "weir-echo-pkg",
        &cfg,
        weir_runtime::HostAllowList::allow_all(),
        &[],
    )
    .expect("load wasm echo component");

    // Same roles via capability bits, over both primitives.
    assert!(wasm.is_source().unwrap() && wasm.is_destination().unwrap());
    assert_eq!(native.is_source().unwrap(), wasm.is_source().unwrap());
    assert_eq!(
        native.is_destination().unwrap(),
        wasm.is_destination().unwrap()
    );

    // spec (typed) — identical.
    assert_eq!(native.spec().unwrap(), wasm.spec().unwrap());

    // discover (typed) — identical.
    assert_eq!(native.discover(&cfg).unwrap(), wasm.discover(&cfg).unwrap());

    // read (v1 streaming) — identical message sequence over both primitives.
    let ctx = ReadContext {
        stream: configured_stream(),
        partition: Partition {
            id: "p0".to_string(),
            bounds: None,
        },
        state: StreamState::default(),
    };
    async fn collect(h: &ConnectorHandle, ctx: &ReadContext) -> Vec<ReadMessage> {
        h.read(ctx)
            .await
            .expect("read")
            .map(|item| item.expect("read item"))
            .collect()
            .await
    }
    assert_eq!(collect(&native, &ctx).await, collect(&wasm, &ctx).await);

    // write (client-streaming) — identical ack over both primitives.
    let wctx = WriteContext {
        stream: configured_stream(),
    };
    let batches = || {
        vec![RecordBatch::Rows(vec![
            "{\"id\":1}".to_string(),
            "{\"id\":2}".to_string(),
        ])]
    };
    assert_eq!(
        native.write(&wctx, batches()).unwrap(),
        wasm.write(&wctx, batches()).unwrap()
    );
}
