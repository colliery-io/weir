//! [[WEIR-T-0175]] / [[WEIR-A-0041]] review trigger: the **hostname-carrying TCP
//! egress** proven end-to-end through weir's `HostAllowList` — a real wasm guest
//! (`wasm-fixtures/tcp-probe`) dials by NAME over `fidius_guest::sockets::tcp`,
//! fidius resolve-and-pins the lookup, and the policy's `authorize_tcp_target`
//! sees `TcpTarget { host: Some(name) }`.
//!
//! MySQL is the wire protocol of choice because the server speaks FIRST: the
//! handshake greeting arrives unsolicited on connect, so the probe needs no
//! MySQL client to prove real bytes flowed. The always-on tests use a local
//! canned-greeting server (byte-faithful MySQL handshake head); the `#[ignore]`d
//! test runs the same probe against the real `mysql` compose service
//! (`docker compose up -d mysql`, or `angreal integration up`).

use std::io::Write;
use std::net::TcpListener;
use std::path::Path;

use futures::StreamExt;
use weir_connector::{
    Config, ConfiguredStream, MappingSpec, Partition, ReadContext, ReadMessage, RecordBatch,
    StreamState, SyncMode, WriteMode,
};
use weir_runtime::{ConnectorHandle, HostAllowList};

/// MySQL protocol-10 handshake head: packet header (3-byte LE length + seq 0),
/// protocol version `0x0a`, then the NUL-terminated server version string. The
/// canned server sends exactly what a real mysqld sends first.
fn canned_mysql_greeting() -> Vec<u8> {
    let mut payload = vec![0x0a];
    payload.extend_from_slice(b"8.4.0-canned\0");
    payload.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // thread id
    payload.extend_from_slice(b"abcdefgh\0"); // auth-plugin-data part 1
    payload.extend_from_slice(&[0xff, 0xf7]); // capability flags (low)
    let mut pkt = vec![payload.len() as u8, 0, 0, 0]; // len (<256) + seq 0
    pkt.extend_from_slice(&payload);
    pkt
}

/// A local server that speaks first, MySQL-style: on each accept, write the
/// canned greeting and hold the socket briefly. Bound via ("localhost", 0) so
/// the listener sits on the same first-resolved candidate the guest dials.
fn start_canned_server() -> (u16, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind(("localhost", 0)).expect("bind canned server");
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        // Serve a handful of connections then exit (tests make 1-2 each).
        for _ in 0..8 {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let _ = stream.write_all(&canned_mysql_greeting());
            let _ = stream.flush();
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    });
    (port, handle)
}

fn stage_probe(root: &Path) {
    weir_wasm_testkit::stage(
        &weir_wasm_testkit::WasmPackage {
            fixture_dir: &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../wasm-fixtures/tcp-probe"),
            wasm_file: "weir_tcp_probe_wasm.wasm",
            pkg_name: "weir-tcp-probe-pkg",
            capabilities: &["tcp"],
        },
        root,
    );
}

fn load_probe(root: &Path, host: &str, port: u16, allowed: &[&str]) -> ConnectorHandle {
    let cfg = Config {
        json: format!("{{\"host\":\"{host}\",\"port\":{port}}}"),
    };
    let policy = HostAllowList {
        allowed_hosts: allowed.iter().map(|s| s.to_string()).collect(),
        inject_headers: Vec::new(),
        credential: None,
    };
    ConnectorHandle::from_wasm_package(root, "weir-tcp-probe-pkg", &cfg, policy, &[])
        .expect("load tcp-probe wasm")
}

/// Drive one `read` and return every message the guest emitted.
fn read_all(source: &ConnectorHandle) -> Vec<ReadMessage> {
    let ctx = ReadContext {
        stream: ConfiguredStream {
            stream: "probe".to_string(),
            sync_mode: SyncMode::FullRefresh,
            cursor_field: None,
            primary_key: None,
            write_mode: WriteMode::Append,
            mapping: MappingSpec::default(),
        },
        partition: Partition {
            id: "p0".to_string(),
            bounds: None,
        },
        state: StreamState {
            cursor: None,
            opaque: Vec::new(),
        },
    };
    // Off-tokio on purpose: wasmtime-wasi does its own block_on inside the guest
    // call, which panics under a live tokio runtime (same rule as the engine's
    // read loop — see weir-engine's sync driver).
    futures::executor::block_on(async {
        let stream = source.read(&ctx).await.expect("open read stream");
        stream
            .map(|m| m.expect("transport"))
            .collect::<Vec<_>>()
            .await
    })
}

/// The record's `first_bytes_hex`, or the Fatal error message.
fn outcome(msgs: &[ReadMessage]) -> Result<String, String> {
    for m in msgs {
        match m {
            ReadMessage::Records(RecordBatch::Rows(rows)) => {
                let v: serde_json::Value = serde_json::from_str(&rows[0]).expect("record json");
                return Ok(v["first_bytes_hex"]
                    .as_str()
                    .expect("hex field")
                    .to_string());
            }
            ReadMessage::Fatal(e) => return Err(e.message.clone()),
            _ => {}
        }
    }
    panic!("probe emitted neither a record nor a Fatal: {}", msgs.len());
}

/// Assert `hex` is a MySQL protocol-10 greeting and return the server version
/// string (payload byte 0 == 0x0a; version is NUL-terminated right after).
fn assert_mysql_greeting(hex: &str) -> String {
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    assert!(bytes.len() > 6, "greeting too short: {} bytes", bytes.len());
    assert_eq!(bytes[4], 0x0a, "payload starts with protocol version 10");
    let version: Vec<u8> = bytes[5..]
        .iter()
        .take_while(|b| **b != 0)
        .copied()
        .collect();
    String::from_utf8(version).expect("server version is ASCII")
}

/// Positive: a NAME-keyed allow-list authorizes a by-name dial, and the guest
/// reads the MySQL-style greeting off the wire — hostname egress end-to-end
/// through `HostAllowList::authorize_tcp_target`.
#[test]
fn name_allowlist_permits_by_name_dial_and_greeting_flows() {
    let (port, _srv) = start_canned_server();
    let tmp = tempfile::TempDir::new().unwrap();
    stage_probe(tmp.path());

    let source = load_probe(tmp.path(), "localhost", port, &["localhost"]);
    let hex = outcome(&read_all(&source)).expect("name-allowed probe succeeds");
    assert_eq!(assert_mysql_greeting(&hex), "8.4.0-canned");
}

/// Negative: an allow-list keyed to a DIFFERENT name denies the dial — the
/// guest sees a connect failure, never bytes.
#[test]
fn wrong_name_allowlist_denies_the_dial() {
    let (port, _srv) = start_canned_server();
    let tmp = tempfile::TempDir::new().unwrap();
    stage_probe(tmp.path());

    let source = load_probe(tmp.path(), "localhost", port, &["db.example.com"]);
    let err = outcome(&read_all(&source)).expect_err("unlisted name must be denied");
    assert!(
        err.contains("connect"),
        "denial surfaces as a connect failure: {err}"
    );
}

/// Fail-closed: a name-keyed allow-list must NOT authorize an IP-literal dial
/// to the very address the name resolves to (`TcpTarget { host: None }`).
#[test]
fn name_allowlist_denies_ip_literal_dial() {
    let (port, _srv) = start_canned_server();
    let tmp = tempfile::TempDir::new().unwrap();
    stage_probe(tmp.path());

    let source = load_probe(tmp.path(), "127.0.0.1", port, &["localhost"]);
    let err = outcome(&read_all(&source)).expect_err("IP-literal dial must be denied");
    assert!(
        err.contains("connect"),
        "denial surfaces as a connect failure: {err}"
    );
}

/// The same probe against a REAL MySQL server (`docker compose up -d mysql`,
/// port override `WEIR_MYSQL_HOST_PORT`): by-name dial through the name-keyed
/// allow-list, and the genuine mysqld greeting comes back.
#[test]
#[ignore = "needs the `mysql` compose service (integration profile)"]
fn real_mysql_greeting_over_hostname_allowlist() {
    let port: u16 = std::env::var("WEIR_MYSQL_HOST_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3306);
    let tmp = tempfile::TempDir::new().unwrap();
    stage_probe(tmp.path());

    let source = load_probe(tmp.path(), "localhost", port, &["localhost"]);
    let hex = outcome(&read_all(&source)).expect("mysql probe succeeds");
    let version = assert_mysql_greeting(&hex);
    assert!(
        version.starts_with("8."),
        "real mysqld advertises its version, got `{version}`"
    );
}
