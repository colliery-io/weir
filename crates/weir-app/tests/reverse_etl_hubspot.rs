//! [[WEIR-T-0074]] (reverse-ETL S4): **HubSpot as a manifest** on the shared `rest-dest`
//! runtime — the proof that a SaaS destination is config, not code ([[WEIR-A-0034]]). Bakes
//! `dest-manifests/hubspot.yaml` → the runtime config, then runs a source → `rest-dest`
//! activation against a **mock HubSpot**: PATCH the email-keyed contact endpoint with a
//! `{"properties": …}` body, a private-app **bearer token injected host-side** ([[WEIR-A-0033]]),
//! a rejected record dead-lettered, and idempotent under replay.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;

use weir_connector::{Config, ConfiguredStream, MappingSpec, SyncMode, WriteMode};
use weir_engine::{Engine, Store};
use weir_manifest::DestinationManifest;
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

/// Mock HubSpot: stores the full raw request keyed by path (so replays upsert the same keys),
/// and rejects `reject@x.com` with a 422 (→ dead-letter). Returns base URL + the keyed store.
fn mock_hubspot() -> (String, Arc<Mutex<HashMap<String, String>>>) {
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
            let (status, body) = if req.contains("reject@x.com") {
                (
                    "422 Unprocessable Entity",
                    r#"{"status":"error","message":"invalid"}"#,
                )
            } else {
                store2.lock().unwrap().insert(path, req.clone());
                ("200 OK", r#"{"id":"hs-1"}"#)
            };
            let resp = format!(
                "HTTP/1.1 {}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                status,
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
    });
    (format!("http://{addr}"), store)
}

fn contacts_stream() -> ConfiguredStream {
    ConfiguredStream {
        stream: "contacts".to_string(),
        sync_mode: SyncMode::FullRefresh,
        cursor_field: None,
        primary_key: Some(vec!["email".to_string()]),
        write_mode: WriteMode::Upsert {
            business_keys: vec!["email".to_string()],
        },
        mapping: MappingSpec::default(),
    }
}

#[test]
fn hubspot_manifest_upserts_contacts_over_wasm() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    stage_both(root);

    // Bake the vendored HubSpot destination manifest → the rest-dest config, then point it at
    // the mock HubSpot (base_url override).
    let manifest_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dest-manifests/hubspot-dest.yaml");
    let m = DestinationManifest::from_yaml(&std::fs::read_to_string(&manifest_path).unwrap())
        .expect("parse hubspot dest manifest");
    let mut dest_cfg = weir_app::dest_object_to_config(&m, "contacts");
    // Sanity: the bake produced the HubSpot shape.
    assert_eq!(dest_cfg["method"], "PATCH");
    assert_eq!(dest_cfg["body_wrap"], "properties");
    assert_eq!(dest_cfg["auth_scheme"], "bearer");

    let (hubspot, store) = mock_hubspot();
    dest_cfg["base_url"] = serde_json::Value::String(hubspot);

    let src = mock_source(
        r#"[{"email":"a@x.com","first_name":"Ada","last_name":"L"},{"email":"reject@x.com","first_name":"No","last_name":"Pe"},{"email":"c@x.com","first_name":"Cy","last_name":"B"}]"#,
    );

    let db = Store::open(root.join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&db);

    let source = ConnectorHandle::from_wasm_package(
        root,
        "weir-rest-pkg",
        &Config {
            json: serde_json::json!({ "base_url": src, "path": "/contacts" }).to_string(),
        },
        HostAllowList::allow_all(),
        &[],
    )
    .expect("load source");

    // The private-app token is injected host-side — never in the manifest or guest config.
    let policy = HostAllowList {
        allowed_hosts: vec!["127.0.0.1".to_string()],
        inject_headers: vec![("authorization".to_string(), "Bearer pat-secret".to_string())],
        credential: None,
    };
    let dest = ConnectorHandle::from_wasm_package(
        root,
        "weir-rest-dest-pkg",
        &Config {
            json: dest_cfg.to_string(),
        },
        policy,
        &[],
    )
    .expect("load hubspot dest");

    for run in 1..=2 {
        let out = engine
            .sync("hs", &contacts_stream(), &source, &dest)
            .unwrap_or_else(|e| panic!("run {run}: {e:?}"));
        assert_eq!(
            out.rows_written, 2,
            "run {run}: two contacts upserted (one rejected)"
        );
    }

    let store = store.lock().unwrap();
    assert_eq!(
        store.len(),
        2,
        "idempotent: replay upserted the same 2 email keys"
    );
    assert_eq!(
        db.dead_letter_count("hs").unwrap(),
        2,
        "the rejected contact dead-lettered each run"
    );

    let ada = store
        .iter()
        .find(|(k, _)| k.contains("a@x.com"))
        .map(|(_, v)| v.clone())
        .expect("a@x.com upserted");
    assert!(
        ada.contains("PATCH /crm/v3/objects/contacts/a@x.com?idProperty=email"),
        "email-keyed upsert URL; got:\n{ada}"
    );
    assert!(
        ada.to_ascii_lowercase()
            .contains("authorization: bearer pat-secret"),
        "bearer injected host-side; got:\n{ada}"
    );
    assert!(
        ada.contains("\"properties\""),
        "HubSpot properties wrap; got:\n{ada}"
    );
    assert!(
        ada.contains("\"firstname\":\"Ada\""),
        "field map (first_name → firstname); got:\n{ada}"
    );
}
