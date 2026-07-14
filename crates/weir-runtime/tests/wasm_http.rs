//! WEIR-T-0023: a codegen'd WASM connector (`rest`) does **live outbound HTTP**
//! via `fidius_guest::http`, brokered by the host `EgressPolicy` (the two-key gate:
//! the package declares the `http` capability + the host supplies a policy). Allow
//! → records flow; deny → the guest sees a transport error. Builds the guest for
//! wasm32-wasip2 at test time (like `wasm_seam`).

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::Command;
use std::thread;

use futures::StreamExt;
use weir_connector::{
    Config, ConfiguredStream, MappingSpec, Partition, ReadContext, ReadMessage, RecordBatch,
    StreamState, SyncMode,
};
use weir_runtime::{ConnectorHandle, HostAllowList};

/// Build the WASM `rest` guest and stage it as a fidius package **with the
/// `http` capability** under `root`.
fn build_and_stage(root: &Path) {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/connectors/rest");
    let status = Command::new("cargo")
        .args(["build", "--target", "wasm32-wasip2", "--release"])
        .current_dir(&fixture)
        .status()
        .expect("cargo build wasm32-wasip2 rest");
    assert!(status.success(), "wasm rest guest build failed");

    let art = fixture.join("target/wasm32-wasip2/release/weir_rest_wasm.wasm");
    let bytes = std::fs::read(&art).unwrap_or_else(|e| panic!("read {}: {e}", art.display()));

    let dir = root.join("weir-rest-pkg");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("package.toml"),
        r#"
[package]
name = "weir-rest-pkg"
version = "0.1.0"
interface = "connector"
interface_version = 1
runtime = "wasm"

[metadata]
category = "test"

[wasm]
component = "weir_rest_wasm.wasm"
capabilities = ["http"]
"#,
    )
    .unwrap();
    std::fs::write(dir.join("weir_rest_wasm.wasm"), bytes).unwrap();
}

/// One-shot mock returning `body` (JSON). Returns the base URL (no trailing slash).
fn mock_http_once(body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{addr}");
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let resp = format!(
                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
    });
    url
}

fn posts_stream() -> ConfiguredStream {
    ConfiguredStream {
        stream: "posts".to_string(),
        sync_mode: SyncMode::Incremental,
        cursor_field: Some("updated_at".to_string()),
        primary_key: Some(vec!["id".to_string()]),
        write_mode: weir_connector::WriteMode::Append,
        mapping: MappingSpec::default(),
    }
}

/// Connector config carrying the target base URL — bound at construction (v1).
fn cfg(base_url: &str) -> Config {
    Config {
        json: format!(
            "{{\"base_url\":\"{base_url}\",\"path\":\"/posts\",\"cursor_field\":\"updated_at\"}}"
        ),
    }
}

fn ctx() -> ReadContext {
    ReadContext {
        stream: posts_stream(),
        partition: Partition {
            id: "p0".to_string(),
            bounds: None,
        },
        state: StreamState::default(),
    }
}

/// Drive the streaming read to completion, collecting all messages. Uses
/// `futures::executor::block_on` (not a tokio runtime) so the thread has no
/// *entered* tokio runtime — wasmtime-wasi's `wasi:http` handler does its own
/// `block_on`, which would panic ("runtime within a runtime") under `#[test]`.
fn read_all(conn: &ConnectorHandle) -> Vec<ReadMessage> {
    futures::executor::block_on(async {
        conn.read(&ctx())
            .await
            .expect("read")
            .map(|item| item.expect("read item"))
            .collect()
            .await
    })
}

#[test]
fn wasm_connector_fetches_live_http_when_egress_allows() {
    // One record (< page size 2) so the paginating guest stops after one request.
    let base = mock_http_once(r#"[{"id":1,"title":"hello","updated_at":"2026-01-01T00:00:00Z"}]"#);

    let tmp = tempfile::TempDir::new().unwrap();
    build_and_stage(tmp.path());

    let policy = HostAllowList {
        allowed_hosts: vec!["127.0.0.1".to_string()],
        inject_headers: vec![],
        credential: None,
    };
    let conn =
        ConnectorHandle::from_wasm_package(tmp.path(), "weir-rest-pkg", &cfg(&base), policy, &[])
            .expect("load wasm rest with egress");

    let msgs = read_all(&conn);
    let rows = msgs
        .iter()
        .find_map(|m| match m {
            ReadMessage::Records(RecordBatch::Rows(r)) => Some(r),
            _ => None,
        })
        .expect("a Records(Rows) message");
    assert_eq!(rows.len(), 1, "fetched the one record over wasi:http");
    assert!(rows[0].contains("\"id\":1") && rows[0].contains("hello"));
    let cursor = msgs.iter().find_map(|m| match m {
        ReadMessage::Checkpoint(s) => Some(s.cursor.as_deref()),
        _ => None,
    });
    assert_eq!(
        cursor,
        Some(Some("2026-01-01T00:00:00Z")),
        "incremental cursor tracked from the fetched record"
    );
}

#[test]
fn wasm_connector_egress_denied_surfaces_error() {
    let tmp = tempfile::TempDir::new().unwrap();
    build_and_stage(tmp.path());

    // Allow-list excludes the target host → the policy refuses before dispatch.
    let policy = HostAllowList {
        allowed_hosts: vec!["127.0.0.1".to_string()],
        inject_headers: vec![],
        credential: None,
    };
    let conn = ConnectorHandle::from_wasm_package(
        tmp.path(),
        "weir-rest-pkg",
        &cfg("http://blocked.invalid"),
        policy,
        &[],
    )
    .expect("load wasm rest with egress");

    let msgs = read_all(&conn);
    assert!(
        msgs.iter().any(|m| matches!(m, ReadMessage::Fatal(_))),
        "denied egress surfaces as a terminal Fatal message, not records"
    );
}
