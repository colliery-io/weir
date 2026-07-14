//! [[WEIR-T-0072]] (reverse-ETL S2): the shared **declarative destination runtime**
//! (`rest-dest`) as a wasm destination, driven through the engine. A rest wasm **source**
//! reads records from a mock API; the engine streams them to the `rest-dest` wasm dest, which
//! shapes each record (field map) and POSTs it to a mock **SaaS** server. Proves: correct
//! method + shaped body over real `wasi:http`, per-record 4xx → dead-letter (not a failed
//! sync), and the accepted count.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::mpsc;
use std::thread;

use weir_connector::{Config, ConfiguredStream, MappingSpec, SyncMode, WriteMode};
use weir_engine::{Engine, Store};
use weir_runtime::{ConnectorHandle, HostAllowList};

/// Stage both wasm guests (rest source + rest-dest destination) under `root`.
fn stage_both(root: &Path) {
    let connectors = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/connectors");
    weir_wasm_testkit::stage(
        &weir_wasm_testkit::WasmPackage {
            fixture_dir: &connectors.join("rest"),
            wasm_file: "weir_rest_wasm.wasm",
            pkg_name: "weir-rest-pkg",
            capabilities: &["http"],
        },
        root,
    );
    weir_wasm_testkit::stage(
        &weir_wasm_testkit::WasmPackage {
            fixture_dir: &connectors.join("rest-dest"),
            wasm_file: "weir_rest_dest_wasm.wasm",
            pkg_name: "weir-rest-dest-pkg",
            capabilities: &["http"],
        },
        root,
    );
}

/// One-shot source: serves `body` (a JSON array of records). Returns the base URL.
fn mock_source(body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
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
    format!("http://{addr}")
}

/// Mock SaaS destination: captures each request (method line + body) and **rejects** any
/// request whose body contains `reject@x.com` with a 400 (→ per-record dead-letter); others
/// get 200. Returns the base URL + a receiver of the captured raw requests.
fn mock_saas() -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut buf = [0u8; 8192];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).into_owned();
            let (status, body) = if req.contains("reject@x.com") {
                ("400 Bad Request", r#"{"error":"invalid email"}"#)
            } else {
                ("200 OK", r#"{"id":"srv-1"}"#)
            };
            let resp = format!(
                "HTTP/1.1 {}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                status,
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
            let _ = tx.send(req);
        }
    });
    (format!("http://{addr}"), rx)
}

fn contacts_stream() -> ConfiguredStream {
    ConfiguredStream {
        stream: "contacts".to_string(),
        sync_mode: SyncMode::FullRefresh,
        cursor_field: None,
        primary_key: Some(vec!["id".to_string()]),
        write_mode: WriteMode::Append,
        mapping: MappingSpec::default(),
    }
}

#[test]
fn wasm_rest_dest_upserts_and_dead_letters() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    stage_both(root);

    // Source: three records; the middle one is destined to be rejected by the SaaS.
    let src = mock_source(
        r#"[{"id":1,"email":"a@x.com","name":"Ada"},{"id":2,"email":"reject@x.com","name":"Bad"},{"id":3,"email":"c@x.com","name":"Cy"}]"#,
    );
    let (saas, rx) = mock_saas();

    let store = Store::open(root.join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let src_cfg = Config {
        json: serde_json::json!({ "base_url": src, "path": "/records" }).to_string(),
    };
    let source = ConnectorHandle::from_wasm_package(
        root,
        "weir-rest-pkg",
        &src_cfg,
        HostAllowList::allow_all(),
        &[],
    )
    .expect("load rest source");

    // Dest: POST each shaped record to the SaaS `/contacts`; map name -> fullname.
    let dest_cfg = Config {
        json: serde_json::json!({
            "base_url": saas, "path": "/contacts", "method": "POST",
            "field_map": { "email": "email", "name": "fullname" },
        })
        .to_string(),
    };
    let dest = ConnectorHandle::from_wasm_package(
        root,
        "weir-rest-dest-pkg",
        &dest_cfg,
        HostAllowList::allow_all(),
        &[],
    )
    .expect("load rest-dest");

    let out = engine
        .sync("revetl", &contacts_stream(), &source, &dest)
        .expect("reverse-ETL sync (rest source → rest-dest)");

    assert_eq!(
        out.rows_written, 2,
        "two records accepted (the third was rejected)"
    );
    assert_eq!(
        store.dead_letter_count("revetl").unwrap(),
        1,
        "the rejected record dead-lettered"
    );

    // The SaaS saw 3 POSTs to /contacts, with the shaped body (name → fullname).
    let reqs: Vec<String> = (0..3)
        .filter_map(|_| rx.recv_timeout(std::time::Duration::from_secs(5)).ok())
        .collect();
    assert_eq!(reqs.len(), 3, "all three records were POSTed");
    assert!(
        reqs.iter().all(|r| r.contains("POST /contacts")),
        "correct method + endpoint"
    );
    let accepted: Vec<&String> = reqs.iter().filter(|r| r.contains("a@x.com")).collect();
    assert!(
        accepted.iter().any(|r| r.contains("\"fullname\":\"Ada\"")),
        "field map applied (name → fullname); got:\n{}",
        accepted.first().map(|s| s.as_str()).unwrap_or("")
    );
}
