//! [[WEIR-I-0035]] F1.7 / [[WEIR-A-0039]]: a **websocket→websocket** resident consumer,
//! driven through the real wasm guest over host-brokered TCP (`fidius_guest::sockets::tcp`,
//! gated by `EgressPolicy::authorize_tcp`). The guest hand-rolls RFC6455; the source's
//! `read` is a genuinely *blocking* live tail (emit per inbound frame — arrival-driven, not
//! a clock). Everything is local (a `tokio-tungstenite` push server on 127.0.0.1) — NO
//! external network.
//!
//! Tests are **close-driven**: the server pushes N frames then closes, so the tail ends with
//! a transient error (the F1.3 requeue signal) after committing N records — deterministic,
//! and it does not rely on interrupting a blocked guest read (drop-to-cancel is only honored
//! *between* frames; a stop cannot interrupt a guest parked in a synchronous socket read —
//! a documented follow-on: guest read timeouts).

use std::cell::Cell;
use std::path::Path;

use futures_util::SinkExt;
use tokio_tungstenite::tungstenite::Message;
use weir_connector::{Config, ConfiguredStream, MappingSpec, SyncMode, WriteMode};
use weir_engine::{Engine, EngineError, Store, SyncOptions, SyncProgress, stop_channel};
use weir_runtime::ConnectorHandle;

fn src_stream(name: &str) -> ConfiguredStream {
    ConfiguredStream {
        stream: name.to_string(),
        sync_mode: SyncMode::FullRefresh,
        cursor_field: None,
        primary_key: None,
        write_mode: WriteMode::Append,
        mapping: MappingSpec::default(),
    }
}

/// A local ws server: on each connection, push `msgs` text frames then **close**. Returns
/// the bound `ws://` URL; the runtime guard keeps it alive.
struct WsServer {
    url: String,
    _rt: tokio::runtime::Runtime,
}

fn start_ws_server(msgs: Vec<String>) -> WsServer {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let listener = rt
        .block_on(async { tokio::net::TcpListener::bind("127.0.0.1:0").await })
        .expect("bind");
    let addr = listener.local_addr().unwrap();
    rt.spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let msgs = msgs.clone();
            tokio::spawn(async move {
                let Ok(mut ws) = tokio_tungstenite::accept_async(stream).await else {
                    return;
                };
                for m in &msgs {
                    let _ = ws.send(Message::Text(m.clone())).await;
                }
                let _ = ws.close(None).await;
            });
        }
    });
    WsServer {
        url: format!("ws://{addr}/"),
        _rt: rt,
    }
}

fn stage_ws_source(root: &Path, conn_stream: &str, url: &str) -> ConnectorHandle {
    weir_wasm_testkit::stage(
        &weir_wasm_testkit::WasmPackage {
            fixture_dir: &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../wasm-fixtures/resident"),
            wasm_file: "weir_resident_wasm.wasm",
            pkg_name: "weir-resident-pkg",
            capabilities: &["tcp"], // host-brokered TCP egress (ADR-0039)
        },
        root,
    );
    let _ = conn_stream;
    let cfg = Config {
        json: format!("{{\"mode\":\"ws\",\"read_url\":\"{url}\"}}"),
    };
    ConnectorHandle::from_wasm_package(
        root,
        "weir-resident-pkg",
        &cfg,
        weir_runtime::HostAllowList::allow_all(),
        &[],
    )
    .expect("load resident-ws wasm")
}

/// Run a resident ws source to its (close-driven) end; return (result, committed-rows).
fn run_ws(
    store: &Store,
    source: &ConnectorHandle,
    dest: &ConnectorHandle,
    conn: &str,
) -> (Result<weir_engine::SyncOutcome, EngineError>, u64) {
    let rows = Cell::new(0u64);
    let (handle, token) = stop_channel();
    let mut cb = |p: SyncProgress| rows.set(p.rows_written);
    let res = Engine::new(store).run_resident(
        conn,
        &src_stream("resident-ws"),
        source,
        dest,
        &SyncOptions::default(),
        &mut cb,
        token,
        None, // arrivals pace it, not a clock
    );
    drop(handle);
    (res, rows.get())
}

/// Emission tracks **arrivals**: 5 frames → 5 committed records; the upstream close then
/// ends the tail with a transient error (the requeue signal).
#[test]
fn resident_ws_emits_one_record_per_frame_arrival() {
    let server = start_ws_server((0..5).map(|i| format!("{{\"n\":{i}}}")).collect());
    let tmp = tempfile::TempDir::new().unwrap();
    let source = stage_ws_source(tmp.path(), "resident-ws", &server.url);
    let store = Store::open(tmp.path().join("weir.db").to_str().unwrap()).expect("store");
    let dest =
        weir_wasm_testkit::load("ArrowSink", &Config { json: "{}".into() }).expect("arrow sink");

    let (res, rows) = run_ws(&store, &source, &dest, "resident-ws");
    assert_eq!(rows, 5, "5 frames arrived → 5 records (arrival-driven)");
    assert!(
        matches!(res, Err(EngineError::Connector(_))),
        "upstream close ends the tail with a transient error, got {res:?}"
    );
}

/// Zero arrivals ⇒ zero records (a clock-driven source could never do this): the server
/// sends nothing and closes; no checkpoints → 0 committed.
#[test]
fn resident_ws_zero_arrivals_zero_records() {
    let server = start_ws_server(vec![]);
    let tmp = tempfile::TempDir::new().unwrap();
    let source = stage_ws_source(tmp.path(), "resident-ws", &server.url);
    let store = Store::open(tmp.path().join("weir.db").to_str().unwrap()).expect("store");
    let dest =
        weir_wasm_testkit::load("ArrowSink", &Config { json: "{}".into() }).expect("arrow sink");

    let (res, rows) = run_ws(&store, &source, &dest, "resident-ws-0");
    assert_eq!(rows, 0, "no arrivals → no records");
    assert!(
        matches!(res, Err(EngineError::Connector(_))),
        "close → transient, got {res:?}"
    );
}

/// Supervision: upstream close is a transient error (F1.3 requeue signal, not a silent
/// death), and a **second** resident run **reconnects** to a fresh upstream and emits again.
#[test]
fn resident_ws_upstream_close_is_transient_then_reconnects() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    let store = Store::open(root.join("weir.db").to_str().unwrap()).expect("store");
    let dest =
        weir_wasm_testkit::load("ArrowSink", &Config { json: "{}".into() }).expect("arrow sink");

    // Run 1: 3 frames then close → transient, 3 committed.
    let server1 = start_ws_server((0..3).map(|i| format!("{{\"a\":{i}}}")).collect());
    let source1 = stage_ws_source(root, "resident-ws", &server1.url);
    let (res1, rows1) = run_ws(&store, &source1, &dest, "resident-ws-rc");
    assert!(
        matches!(res1, Err(EngineError::Connector(_))),
        "close 1 → transient, got {res1:?}"
    );
    assert_eq!(rows1, 3, "run 1 committed 3");

    // Run 2: RECONNECT to a fresh upstream (2 frames) → emits 2. Proves reconnect-on-restart.
    let server2 = start_ws_server((0..2).map(|i| format!("{{\"b\":{i}}}")).collect());
    let source2 = stage_ws_source(root, "resident-ws", &server2.url);
    let (_res2, rows2) = run_ws(&store, &source2, &dest, "resident-ws-rc");
    assert_eq!(rows2, 2, "reconnected run emits the 2 new arrivals");
}
