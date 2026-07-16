//! [[WEIR-T-0157]]: the `snowflake-dest` wasm **destination** driven through the engine —
//! a rest wasm source reads records from a mock API; the engine streams them to the
//! snowflake-dest guest, which compiles chunked SQL API statements (CREATE TABLE +
//! multi-row INSERT / keyed MERGE) and POSTs them to a mock Snowflake SQL API over real
//! `wasi:http`. Proves: table ensure, batched statements (not per-record requests),
//! MERGE-with-dedup for upsert, and `202 → statementHandle → poll` settlement.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::mpsc;
use std::thread;

use weir_connector::{Config, ConfiguredStream, MappingSpec, SyncMode, WriteMode};
use weir_engine::{Engine, Store};
use weir_runtime::{ConnectorHandle, HostAllowList};

/// Stage both wasm guests (rest source + snowflake-dest destination) under `root`.
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
            fixture_dir: &connectors.join("snowflake"),
            wasm_file: "weir_snowflake_wasm.wasm",
            pkg_name: "weir-snowflake-pkg",
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

/// Read a full HTTP/1.1 request — content-length or chunked (the guest's `wasi:http`
/// POSTs are chunked, [[WEIR-T-0154]]) — so the mock answers only after the client
/// finished sending. Mirrors `wasm_http_engine::read_full_request`.
fn read_full_request(stream: &mut std::net::TcpStream) -> String {
    stream
        .set_read_timeout(Some(std::time::Duration::from_millis(500)))
        .ok();
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                data.extend_from_slice(&buf[..n]);
                if let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&data[..end]);
                    if head
                        .to_ascii_lowercase()
                        .contains("transfer-encoding: chunked")
                    {
                        if data.ends_with(b"0\r\n\r\n") {
                            break;
                        }
                        continue;
                    }
                    let clen = head
                        .lines()
                        .find_map(|l| {
                            l.split_once(':')
                                .filter(|(k, _)| k.trim().eq_ignore_ascii_case("content-length"))
                                .map(|(_, v)| v)
                        })
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if data.len() >= end + 4 + clen {
                        break;
                    }
                }
            }
            Err(_) => break,
        }
    }
    String::from_utf8_lossy(&data).into_owned()
}

/// A captured request's body, de-chunked when needed. Mirrors `wasm_http_engine`.
fn request_body(req: &str) -> String {
    let Some((head, raw)) = req.split_once("\r\n\r\n") else {
        return String::new();
    };
    if !head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        return raw.to_string();
    }
    let mut out = String::new();
    let mut rest = raw;
    loop {
        let Some((size_line, tail)) = rest.split_once("\r\n") else {
            break;
        };
        let Ok(size) = usize::from_str_radix(size_line.trim(), 16) else {
            break;
        };
        if size == 0 || tail.len() < size {
            break;
        }
        out.push_str(&tail[..size]);
        rest = tail.get(size + 2..).unwrap_or("");
    }
    out
}

/// Mock Snowflake SQL API: captures every request (raw text) and answers each POST to
/// `/api/v2/statements` with `first_status` once, then 200s; GET polls answer 200.
fn mock_sql_api(first_status: u16) -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut first = true;
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { break };
            let req = read_full_request(&mut s);
            let is_post = req.starts_with("POST");
            let status = if is_post && first {
                first = false;
                first_status
            } else {
                200
            };
            let body = r#"{"statementHandle":"h-1","message":"ok"}"#;
            let resp = format!(
                "HTTP/1.1 {} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                status,
                body.len(),
                body
            );
            let _ = s.write_all(resp.as_bytes());
            let _ = s.flush();
            let _ = tx.send(req);
        }
    });
    (format!("http://{addr}"), rx)
}

fn stream(write_mode: WriteMode) -> ConfiguredStream {
    ConfiguredStream {
        stream: "contacts".to_string(),
        sync_mode: SyncMode::FullRefresh,
        cursor_field: None,
        primary_key: Some(vec!["id".to_string()]),
        write_mode,
        mapping: MappingSpec::default(),
    }
}

fn run_sync(write_mode: WriteMode, records: &'static str, sf_status: u16) -> (u64, Vec<String>) {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    stage_both(root);
    let src = mock_source(records);
    let (sf, rx) = mock_sql_api(sf_status);

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

    let dest_cfg = Config {
        json: serde_json::json!({
            "account": "myorg-acct1",
            "database": "demo_db", "schema": "public", "warehouse": "compute_xs",
            "base_url": sf,
        })
        .to_string(),
    };
    let dest = ConnectorHandle::from_wasm_package(
        root,
        "weir-snowflake-pkg",
        &dest_cfg,
        HostAllowList::allow_all(),
        &[],
    )
    .expect("load snowflake-dest");

    let out = engine
        .sync("sf-conn", &stream(write_mode), &source, &dest)
        .expect("engine sync (rest source → snowflake-dest)");

    let mut reqs = Vec::new();
    while let Ok(r) = rx.recv_timeout(std::time::Duration::from_millis(1500)) {
        reqs.push(r);
    }
    (out.rows_written, reqs)
}

/// Append mode: one CREATE TABLE, then ONE multi-row INSERT for the whole batch —
/// never a request per record.
#[test]
fn snowflake_dest_appends_with_batched_insert() {
    let (rows, reqs) = run_sync(
        WriteMode::Append,
        r#"[{"id":1,"email":"a@x.com","score":1.5},{"id":2,"email":"b@x.com","score":2.0},{"id":3,"email":"c@x.com","score":3.25}]"#,
        200,
    );
    assert_eq!(rows, 3);
    let bodies: Vec<serde_json::Value> = reqs
        .iter()
        .filter(|r| r.starts_with("POST"))
        .map(|r| serde_json::from_str(&request_body(r)).expect("statement body JSON"))
        .collect();
    assert_eq!(
        bodies.len(),
        2,
        "CREATE TABLE + one batched INSERT, got: {bodies:?}"
    );
    let create = bodies[0]["statement"].as_str().unwrap();
    assert!(
        create.starts_with(r#"CREATE TABLE IF NOT EXISTS "DEMO_DB"."PUBLIC"."CONTACTS""#),
        "table ensured from the stream name; got: {create}"
    );
    assert!(create.contains(r#""SCORE" DOUBLE"#) && create.contains(r#""ID" NUMBER"#));
    let insert = bodies[1]["statement"].as_str().unwrap();
    assert!(insert.starts_with(r#"INSERT INTO "DEMO_DB"."PUBLIC"."CONTACTS""#));
    assert_eq!(
        insert.matches("(?, ?, ?)").count(),
        3,
        "3 rows in one statement"
    );
    assert_eq!(
        bodies[1]["bindings"]["4"]["value"], "b@x.com",
        "row-major binds"
    );
    assert_eq!(bodies[1]["warehouse"], "COMPUTE_XS", "session context");
}

/// Upsert mode: MERGE keyed on the business key, with in-batch duplicates deduped
/// (last wins) so replay/dup delivery cannot double-write.
#[test]
fn snowflake_dest_upserts_with_keyed_merge_and_dedup() {
    let (rows, reqs) = run_sync(
        WriteMode::Upsert {
            business_keys: vec!["email".to_string()],
        },
        r#"[{"id":1,"email":"a@x.com"},{"id":2,"email":"b@x.com"},{"id":9,"email":"a@x.com"}]"#,
        200,
    );
    assert_eq!(rows, 2, "duplicate key deduped before the MERGE");
    let bodies: Vec<serde_json::Value> = reqs
        .iter()
        .filter(|r| r.starts_with("POST"))
        .map(|r| serde_json::from_str(&request_body(r)).expect("statement body JSON"))
        .collect();
    let merge = bodies
        .iter()
        .find_map(|b| {
            b["statement"]
                .as_str()
                .filter(|s| s.starts_with("MERGE"))
                .map(str::to_string)
        })
        .expect("a MERGE statement");
    assert!(
        merge.contains(r#"ON t."EMAIL" = s."EMAIL""#),
        "keyed on email: {merge}"
    );
    assert_eq!(
        merge.matches("(?, ?)").count(),
        2,
        "two deduped source rows: {merge}"
    );
    // Last write wins: the surviving a@x.com row carries id 9.
    let binds = &bodies.last().unwrap()["bindings"];
    let bound: Vec<String> = (1..=4)
        .map(|i| {
            binds[i.to_string()]["value"]
                .as_str()
                .unwrap_or("")
                .to_string()
        })
        .collect();
    assert!(bound.contains(&"9".to_string()), "last dup won: {bound:?}");
}

/// A `202` on the statement POST polls `GET /api/v2/statements/<handle>` to settlement.
#[test]
fn snowflake_dest_polls_async_statement_to_settlement() {
    let (rows, reqs) = run_sync(WriteMode::Append, r#"[{"id":1,"email":"a@x.com"}]"#, 202);
    assert_eq!(rows, 1, "sync settles after the poll");
    assert!(
        reqs.iter()
            .any(|r| r.starts_with("GET /api/v2/statements/h-1")),
        "the 202 handle was polled; requests:\n{}",
        reqs.iter()
            .map(|r| r.lines().next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

// ── Source side ([[WEIR-T-0158]]) ────────────────────────────────────────────────────

/// Mock SQL API for **reads**: a POST whose statement has no `WHERE` gets a 2-column,
/// 2-partition result (one row inline, one via `GET ?partition=1`); a POST with a
/// `WHERE` (the incremental resume) gets an empty result. Captures every request.
fn mock_sf_source() -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { break };
            let req = read_full_request(&mut s);
            let body = if req.starts_with("GET") {
                // A result-partition fetch.
                r#"{"data":[["b@x.com","2026-01-02"]]}"#.to_string()
            } else {
                let stmt = serde_json::from_str::<serde_json::Value>(&request_body(&req))
                    .ok()
                    .and_then(|b| b["statement"].as_str().map(str::to_string))
                    .unwrap_or_default();
                if stmt.contains("WHERE") {
                    // Incremental resume: nothing newer than the cursor.
                    r#"{"statementHandle":"h-2","resultSetMetaData":{"rowType":[{"name":"EMAIL"},{"name":"UPDATED_AT"}],"partitionInfo":[{}]},"data":[]}"#.to_string()
                } else {
                    r#"{"statementHandle":"h-1","resultSetMetaData":{"rowType":[{"name":"EMAIL"},{"name":"UPDATED_AT"}],"partitionInfo":[{},{}]},"data":[["a@x.com","2026-01-01"]]}"#.to_string()
                }
            };
            let resp = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = s.write_all(resp.as_bytes());
            let _ = s.flush();
            let _ = tx.send(req);
        }
    });
    (format!("http://{addr}"), rx)
}

/// The source read: array rows across BOTH result partitions land as lowercased JSON
/// object records, and a second sync resumes incrementally — the cursor from sync 1's
/// checkpoint arrives as the bound `WHERE` lower-bound in sync 2's statement.
#[test]
fn snowflake_source_reads_partitions_and_resumes_incrementally() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    stage_both(root);
    let (sf, rx) = mock_sf_source();

    let store = Store::open(root.join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let src_cfg = Config {
        json: serde_json::json!({
            "account": "myorg-acct1",
            "database": "demo_db", "schema": "public", "warehouse": "compute_xs",
            "table": "contacts",
            "base_url": sf,
        })
        .to_string(),
    };
    let source = ConnectorHandle::from_wasm_package(
        root,
        "weir-snowflake-pkg",
        &src_cfg,
        HostAllowList::allow_all(),
        &[],
    )
    .expect("load snowflake source");
    let dest = weir_wasm_testkit::load(
        "ArrowSink",
        &Config {
            json: "{}".to_string(),
        },
    )
    .unwrap();

    let cs = ConfiguredStream {
        stream: "contacts".to_string(),
        sync_mode: SyncMode::Incremental,
        cursor_field: Some("updated_at".to_string()),
        primary_key: Some(vec!["email".to_string()]),
        write_mode: WriteMode::Append,
        mapping: MappingSpec::default(),
    };

    let out = engine
        .sync("sf-src-conn", &cs, &source, &dest)
        .expect("first sync (full read, 2 partitions)");
    assert_eq!(
        out.rows_written, 2,
        "inline partition 0 + fetched partition 1"
    );

    let out2 = engine
        .sync("sf-src-conn", &cs, &source, &dest)
        .expect("second sync (incremental resume)");
    assert_eq!(out2.rows_written, 0, "nothing newer than the cursor");

    let reqs: Vec<String> =
        std::iter::from_fn(|| rx.recv_timeout(std::time::Duration::from_millis(1500)).ok())
            .collect();
    // Sync 1: an ordered full read + the partition fetch.
    let first = reqs
        .iter()
        .find(|r| r.starts_with("POST"))
        .map(|r| request_body(r))
        .expect("first statement");
    let first: serde_json::Value = serde_json::from_str(&first).unwrap();
    let stmt1 = first["statement"].as_str().unwrap();
    assert!(
        stmt1.contains(r#""DEMO_DB"."PUBLIC"."CONTACTS""#),
        "reads the configured table: {stmt1}"
    );
    assert!(
        stmt1.ends_with(r#"ORDER BY "UPDATED_AT""#) && !stmt1.contains("WHERE"),
        "first run is a full ordered read: {stmt1}"
    );
    assert!(
        reqs.iter()
            .any(|r| r.starts_with("GET") && r.contains("partition=1")),
        "partition 1 fetched"
    );
    // Sync 2: the checkpointed cursor came back as the bound lower-bound.
    let resume = reqs
        .iter()
        .filter(|r| r.starts_with("POST"))
        .map(|r| request_body(r))
        .filter_map(|b| serde_json::from_str::<serde_json::Value>(&b).ok())
        .find(|b| b["statement"].as_str().is_some_and(|s| s.contains("WHERE")))
        .expect("incremental resume statement");
    assert!(
        resume["statement"]
            .as_str()
            .unwrap()
            .contains(r#"WHERE "UPDATED_AT" > ?"#),
        "cursor lower-bound bound as a param"
    );
    assert_eq!(
        resume["bindings"]["1"]["value"], "2026-01-02",
        "the max cursor from sync 1 (partition 1's row) resumed sync 2"
    );
}
