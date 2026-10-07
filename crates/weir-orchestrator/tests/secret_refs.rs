//! [[WEIR-T-0202]]: `env:` / `file:` secret references resolve host-side on EACH run.
//! A real wasm `rest` source runs twice through the relay + in-process executor
//! against a loopback server that records the `Authorization` header. Between the two
//! runs only the environment variable (or the file) changes — the work spec is the same
//! — and the second run sends the new value ([[WEIR-A-0037]]: re-read per run).

mod common;
use common::wasm_ref;

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use weir_connector::{Config, ConfiguredStream, MappingSpec, SyncMode, WriteMode};
use weir_engine::Store;
use weir_orchestrator::{InProcessExecutor, Relay, WorkSpec, Worker, WorkerConfig};

const BODY: &str = r#"[{"id":1,"title":"hi","updated_at":"2026-01-01T00:00:00Z"}]"#;

/// A loopback HTTP server: answers every request with [`BODY`] and sends the
/// `Authorization` header of each request (lowercased line) on the channel.
fn auth_recorder() -> (String, Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut data = Vec::new();
            let mut buf = [0u8; 4096];
            while !data.windows(4).any(|w| w == b"\r\n\r\n") {
                match stream.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => data.extend_from_slice(&buf[..n]),
                }
            }
            let head = String::from_utf8_lossy(&data).to_ascii_lowercase();
            let auth = head
                .lines()
                .find(|l| l.starts_with("authorization:"))
                .unwrap_or("")
                .trim()
                .to_string();
            let _ = tx.send(auth);
            let resp = format!(
                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{BODY}",
                BODY.len()
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
    });
    (url, rx)
}

fn spec(connection: &str, source_cfg: String) -> WorkSpec {
    WorkSpec {
        connection: connection.to_string(),
        tenant: "default".to_string(),
        stream: ConfiguredStream {
            stream: "posts".to_string(),
            sync_mode: SyncMode::FullRefresh,
            cursor_field: None,
            primary_key: None,
            write_mode: WriteMode::Append,
            mapping: MappingSpec::default(),
        },
        source: wasm_ref("Rest"),
        dest: wasm_ref("ArrowSink"),
        source_config: Config { json: source_cfg },
        dest_config: Config {
            json: "{}".to_string(),
        },
        state_key: None,
        seed_cursor: None,
        partition: None,
        execution_mode: Default::default(),
    }
}

fn setup() -> (tempfile::TempDir, Arc<Store>, Relay) {
    let tmp = tempfile::TempDir::new().unwrap();
    let store = Arc::new(Store::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let relay = Relay::new(Arc::clone(&store)).unwrap();
    (tmp, store, relay)
}

/// Plan `s`, drain it with one worker, and return the end state + error of the unit.
async fn run(relay: &Relay, store: &Arc<Store>, s: &WorkSpec) -> (String, Option<String>) {
    let id = relay.plan(s).unwrap();
    let executor = InProcessExecutor::new(Arc::clone(store), relay.clone());
    Worker::new(
        relay.clone(),
        executor,
        WorkerConfig {
            owner: "secret-refs".to_string(),
            max_attempts: 1,
            base_delay: Duration::ZERO,
            lease: Duration::from_secs(30),
            heartbeat: Duration::ZERO,
            ..Default::default()
        },
    )
    .run_until_idle()
    .await
    .unwrap();
    let unit = relay
        .history(&s.tenant, &s.connection)
        .unwrap()
        .into_iter()
        .find(|u| u.id == id)
        .expect("the planned unit");
    // The stored spec keeps the reference text, never the resolved value.
    let stored = relay.load(id).unwrap().expect("stored spec");
    assert_eq!(stored.source_config.json, s.source_config.json);
    (unit.state, unit.error)
}

fn next_auth(rx: &Receiver<String>) -> String {
    rx.recv_timeout(Duration::from_secs(10))
        .expect("the source made a request")
}

#[tokio::test]
async fn env_reference_rotation_takes_effect_on_the_next_run() {
    let (_tmp, store, relay) = setup();
    let (base, auth) = auth_recorder();
    let var = "WEIR_T0202_ROTATION_TOKEN";
    let cfg = format!(
        r#"{{"base_url":"{base}","path":"/posts","auth_scheme":"bearer","api_key":"env:{var}"}}"#
    );
    let s = spec("secret-ref-env", cfg);

    // SAFETY: the variable name is unique to this test binary.
    unsafe { std::env::set_var(var, "token-one") };
    let (state, err) = run(&relay, &store, &s).await;
    assert_eq!(state, "done", "{err:?}");
    assert_eq!(next_auth(&auth), "authorization: bearer token-one");

    // Rotate the variable only — the same spec runs again.
    unsafe { std::env::set_var(var, "token-two") };
    let (state, err) = run(&relay, &store, &s).await;
    assert_eq!(state, "done", "{err:?}");
    assert_eq!(
        next_auth(&auth),
        "authorization: bearer token-two",
        "the second run re-reads the variable"
    );

    // Unset: the run fails with an error that names the field and the reference.
    unsafe { std::env::remove_var(var) };
    let (state, err) = run(&relay, &store, &s).await;
    assert_eq!(state, "failed");
    let err = err.expect("an error");
    assert!(
        err.contains("`api_key`") && err.contains(&format!("`env:{var}`")),
        "{err}"
    );
    assert!(
        !err.contains("token-one") && !err.contains("token-two"),
        "{err}"
    );
}

#[tokio::test]
async fn file_reference_rotation_takes_effect_on_the_next_run() {
    let (_tmp, store, relay) = setup();
    let (base, auth) = auth_recorder();
    let dir = tempfile::TempDir::new().unwrap();
    let file = dir.path().join("token");
    let cfg = format!(
        r#"{{"base_url":"{base}","path":"/posts","auth_scheme":"bearer","api_key":"file:{}"}}"#,
        file.display()
    );
    let s = spec("secret-ref-file", cfg);

    std::fs::write(&file, "file-one\n").unwrap();
    let (state, err) = run(&relay, &store, &s).await;
    assert_eq!(state, "done", "{err:?}");
    assert_eq!(next_auth(&auth), "authorization: bearer file-one");

    std::fs::write(&file, "file-two\n").unwrap();
    let (state, err) = run(&relay, &store, &s).await;
    assert_eq!(state, "done", "{err:?}");
    assert_eq!(next_auth(&auth), "authorization: bearer file-two");

    std::fs::remove_file(&file).unwrap();
    let (state, err) = run(&relay, &store, &s).await;
    assert_eq!(state, "failed");
    let err = err.expect("an error");
    assert!(err.contains("`api_key`") && err.contains("`file:"), "{err}");
    assert!(!err.contains("file-two"), "{err}");
}
