//! [[WEIR-T-0073]] (reverse-ETL S3): the full activation flow — a source (warehouse
//! stand-in) → engine **mapping** → `rest-dest` **upsert** to a SaaS — proven **idempotent
//! under replay**. A `WriteMode::Upsert` stream makes the dest PATCH a keyed URL, so running
//! the sync twice against a stateful mock SaaS yields the same records, not duplicates. The
//! Postgres *source* (the literal warehouse) is a config swap — it already has a Source role
//! and its own tests; this proves the reverse-ETL semantics hermetically.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;

use weir_connector::{
    ComputeExpr, Config, ConfiguredStream, MappingOp, MappingSpec, SyncMode, WriteMode,
};
use weir_engine::{Engine, Store};
use weir_runtime::{ConnectorHandle, HostAllowList};

fn stage_both(root: &Path) {
    let connectors = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/connectors");
    for (dir, wasm, pkg) in [
        ("rest", "weir_rest_wasm.wasm", "weir-rest-pkg"),
        (
            "rest-dest",
            "weir_rest_dest_wasm.wasm",
            "weir-rest-dest-pkg",
        ),
    ] {
        weir_wasm_testkit::stage(
            &weir_wasm_testkit::WasmPackage {
                fixture_dir: &connectors.join(dir),
                wasm_file: wasm,
                pkg_name: pkg,
                capabilities: &["http"],
            },
            root,
        );
    }
}

/// Source that re-serves `body` (a JSON record array) on **every** connection, so replays
/// re-read the warehouse.
fn mock_source(body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
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
    format!("http://{addr}")
}

/// Stateful mock SaaS: upserts by the request path (the keyed URL) into a shared store.
/// Returns the base URL + the store, so a test can assert the final state after replays.
fn mock_saas() -> (String, Arc<Mutex<HashMap<String, String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let store = Arc::new(Mutex::new(HashMap::new()));
    let store2 = store.clone();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut buf = [0u8; 8192];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).into_owned();
            let path = req.split_whitespace().nth(1).unwrap_or("").to_string();
            let body = req.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
            store2.lock().unwrap().insert(path, body);
            let resp = "HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}";
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
    });
    (format!("http://{addr}"), store)
}

fn upsert_stream() -> ConfiguredStream {
    ConfiguredStream {
        stream: "contacts".to_string(),
        sync_mode: SyncMode::FullRefresh,
        cursor_field: None,
        primary_key: Some(vec!["email".to_string()]),
        write_mode: WriteMode::Upsert {
            business_keys: vec!["email".to_string()],
        },
        // Mapping en route: stamp a provenance field before the SaaS write.
        mapping: MappingSpec {
            ops: vec![MappingOp::Compute {
                field: "source".to_string(),
                value: ComputeExpr::Const("weir".to_string()),
            }],
        },
    }
}

#[test]
fn reverse_etl_upsert_is_idempotent_under_replay() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    stage_both(root);

    let src = mock_source(
        r#"[{"email":"a@x.com","name":"Ada"},{"email":"b@x.com","name":"Bo"},{"email":"c@x.com","name":"Cy"}]"#,
    );
    let (saas, store) = mock_saas();

    let db = Store::open(root.join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&db);

    let source = ConnectorHandle::from_wasm_package(
        root,
        "weir-rest-pkg",
        &Config {
            json: serde_json::json!({ "base_url": src, "path": "/rows" }).to_string(),
        },
        HostAllowList::allow_all(),
        &[],
    )
    .expect("load source");

    // No `method` → the Upsert stream drives PATCH to the keyed URL. Identity field map
    // (empty) so the mapping-added `source` field passes through to the SaaS body.
    let dest = ConnectorHandle::from_wasm_package(
        root,
        "weir-rest-dest-pkg",
        &Config {
            json: serde_json::json!({ "base_url": saas, "path": "/contacts/{{ record.email }}" })
                .to_string(),
        },
        HostAllowList::allow_all(),
        &[],
    )
    .expect("load dest");

    // Run twice (activation + replay).
    for run in 1..=2 {
        let out = engine
            .sync("revetl", &upsert_stream(), &source, &dest)
            .unwrap_or_else(|e| panic!("run {run} sync: {e:?}"));
        assert_eq!(out.rows_written, 3, "run {run}: three records upserted");
    }

    let store = store.lock().unwrap();
    assert_eq!(
        store.len(),
        3,
        "idempotent: replay upserted the same 3 keys, no duplicates"
    );
    assert!(
        store.contains_key("/contacts/a@x.com"),
        "keyed upsert URL used; keys: {:?}",
        store.keys().collect::<Vec<_>>()
    );
    let ada = &store["/contacts/a@x.com"];
    assert!(
        ada.contains("\"source\":\"weir\""),
        "mapping applied en route; body: {ada}"
    );
    assert!(ada.contains("Ada"), "record payload preserved; body: {ada}");
}
