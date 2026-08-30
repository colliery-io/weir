//! [[WEIR-T-0161]]: the `mssql` connector as a wasm guest doing real TDS to a live SQL
//! Server (`fidius_guest::sockets::tcp` + tiberius over a runtime-free block_on), driven
//! through the engine — discover, full-refresh, and cursor-incremental reads against the
//! seeded `weir_demo.dbo.contacts`. `#[ignore]`: needs the integration MSSQL
//! (`angreal integration up` / `docker compose up -d --wait`). Run with
//! `cargo test -p weir-engine --test wasm_mssql_engine -- --ignored --test-threads=1`.

use std::net::SocketAddr;
use std::path::Path;

use weir_connector::fidius::{EgressDenied, EgressPolicy};
use weir_connector::{Config, ConfiguredStream, MappingSpec, SyncMode, WriteMode};
use weir_engine::{Engine, Store};
use weir_runtime::ConnectorHandle;

/// `host:port` of the integration SQL Server — `WEIR_TEST_MSSQL_HOST`/`_PORT` override the
/// compose defaults (loopback:1433).
fn mssql_host() -> String {
    std::env::var("WEIR_TEST_MSSQL_HOST").unwrap_or_else(|_| "localhost".to_string())
}
fn mssql_port() -> u16 {
    std::env::var("WEIR_TEST_MSSQL_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(1433)
}

/// TCP-only egress pinned to the MSSQL port (TDS is not HTTP).
struct TcpOnly(u16);
impl EgressPolicy for TcpOnly {
    fn authorize(
        &self,
        _parts: &mut weir_connector::fidius::http_types::request::Parts,
    ) -> Result<(), EgressDenied> {
        Err(EgressDenied::new(
            "mssql connector does not use http egress",
        ))
    }
    fn authorize_tcp(&self, addr: &SocketAddr) -> Result<(), EgressDenied> {
        if addr.port() == self.0 {
            Ok(())
        } else {
            Err(EgressDenied::new(format!("tcp to {addr} not allowed")))
        }
    }
}

fn mssql(root: &Path, table: Option<&str>) -> ConnectorHandle {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/connectors/mssql");
    weir_wasm_testkit::stage(
        &weir_wasm_testkit::WasmPackage {
            fixture_dir: &fixture,
            wasm_file: "weir_mssql_wasm.wasm",
            pkg_name: "weir-mssql-pkg",
            capabilities: &["tcp"],
        },
        root,
    );
    let mut cfg = serde_json::json!({
        "host": mssql_host(),
        "port": mssql_port(),
        "database": "weir_demo",
        "user": "sa",
        "password": "weirStrong!Pass1",
        // The plain compose service has Force Encryption off; the connector's
        // default is `require` ([[WEIR-A-0041]]) — TLS gates live below.
        "tls_mode": "disable",
    });
    if let Some(t) = table {
        cfg["table"] = t.into();
    }
    ConnectorHandle::from_wasm_package(
        root,
        "weir-mssql-pkg",
        &Config {
            json: cfg.to_string(),
        },
        TcpOnly(mssql_port()),
        &[],
    )
    .expect("load mssql wasm")
}

fn arrow() -> ConnectorHandle {
    weir_wasm_testkit::load(
        "ArrowSink",
        &Config {
            json: "{}".to_string(),
        },
    )
    .expect("arrow")
}

fn stream(sync: SyncMode, cursor: Option<&str>) -> ConfiguredStream {
    ConfiguredStream {
        stream: "dbo.contacts".to_string(),
        sync_mode: sync,
        cursor_field: cursor.map(str::to_string),
        primary_key: Some(vec!["id".to_string()]),
        write_mode: WriteMode::Append,
        mapping: MappingSpec::default(),
    }
}

fn store(tmp: &Path, name: &str) -> Store {
    Store::open(tmp.join(format!("{name}.db")).to_str().unwrap()).expect("store")
}

/// Discover lists the seeded base table.
#[test]
#[ignore = "needs the integration MSSQL (docker compose up)"]
fn mssql_wasm_discovers_tables() {
    let tmp = tempfile::TempDir::new().unwrap();
    let src = mssql(tmp.path(), None);
    let cfg = Config {
        json: serde_json::json!({
            "host": mssql_host(), "port": mssql_port(),
            "database": "weir_demo", "user": "sa", "password": "weirStrong!Pass1",
        })
        .to_string(),
    };
    let outcome = src.discover(&cfg).expect("discover");
    let names: Vec<String> = match outcome {
        weir_connector::DiscoverOutcome::Catalog(c) => {
            c.streams.into_iter().map(|s| s.name).collect()
        }
        _ => Vec::new(),
    };
    assert!(
        names.iter().any(|n| n == "dbo.contacts"),
        "discover lists the seeded table; got {names:?}"
    );
}

/// Full-refresh reads all seeded rows through the guest → arrow-sink.
#[test]
#[ignore = "needs the integration MSSQL (docker compose up)"]
fn mssql_wasm_full_refresh_reads_all_rows() {
    let tmp = tempfile::TempDir::new().unwrap();
    let src = mssql(tmp.path(), None);
    let out = Engine::new(&store(tmp.path(), "r"))
        .sync("r", &stream(SyncMode::FullRefresh, None), &src, &arrow())
        .expect("full refresh read");
    assert_eq!(
        out.rows_written, 3,
        "3 seeded contacts read via FOR JSON PATH"
    );
}

/// Incremental reads rows past the cursor and advances it; a second run from the
/// checkpoint reads nothing new.
#[test]
#[ignore = "needs the integration MSSQL (docker compose up)"]
fn mssql_wasm_incremental_advances_and_resumes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let src = mssql(tmp.path(), None);
    let inc = stream(SyncMode::Incremental, Some("updated_at"));

    let out = Engine::new(&store(tmp.path(), "inc"))
        .sync("inc", &inc, &src, &arrow())
        .expect("incremental read");
    assert_eq!(out.rows_written, 3, "first incremental run reads all rows");

    // Persisted cursor is the max updated_at (FOR JSON emits ISO 8601; fractional-second
    // precision varies by datetime2 scale, so match the date-time prefix).
    let s = store(tmp.path(), "inc");
    let cursor = s.cursor("inc", "dbo.contacts").expect("cursor");
    assert!(
        cursor
            .as_deref()
            .is_some_and(|c| c.starts_with("2026-01-03T00:00:00")),
        "cursor advanced to the max updated_at; got {cursor:?}"
    );
    let out2 = Engine::new(&s)
        .sync("inc", &inc, &src, &arrow())
        .expect("incremental resume");
    assert_eq!(
        out2.rows_written, 0,
        "resume from the cursor reads nothing new"
    );
}

// ---- TLS wire gates ([[WEIR-T-0177]] / [[WEIR-A-0041]]) — against the
// `mssql-tls` compose service (Force Encryption on, cert = the ephemeral
// harness CA's localhost-SAN leaf; `scripts/gen-test-tls.sh`). The TDS 7.x
// handshake runs INSIDE PRELOGIN via the weir-tiberius fork. ----

/// The Force-Encryption SQL Server port (`WEIR_MSSQL_TLS_HOST_PORT`, default 11433).
fn mssql_tls_port() -> u16 {
    std::env::var("WEIR_MSSQL_TLS_HOST_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(11433)
}

/// The harness CA (inline PEM for `tls_ca_pem`).
fn harness_ca() -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/weir-tls-certs/ca.crt");
    std::fs::read_to_string(&p).unwrap_or_else(|e| {
        panic!(
            "read {} ({e}) — run scripts/gen-test-tls.sh first",
            p.display()
        )
    })
}

/// Load the mssql guest against the TLS service with explicit tls settings.
fn mssql_tls(root: &Path, host: &str, tls: serde_json::Value) -> ConnectorHandle {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/connectors/mssql");
    weir_wasm_testkit::stage(
        &weir_wasm_testkit::WasmPackage {
            fixture_dir: &fixture,
            wasm_file: "weir_mssql_wasm.wasm",
            pkg_name: "weir-mssql-pkg",
            capabilities: &["tcp"],
        },
        root,
    );
    let mut cfg = serde_json::json!({
        "host": host,
        "port": mssql_tls_port(),
        "database": "weir_demo",
        "user": "sa",
        "password": "weirStrong!Pass1",
    });
    for (k, v) in tls.as_object().unwrap() {
        cfg[k] = v.clone();
    }
    ConnectorHandle::from_wasm_package(
        root,
        "weir-mssql-pkg",
        &Config {
            json: cfg.to_string(),
        },
        TcpOnly(mssql_tls_port()),
        &[],
    )
    .expect("load mssql wasm")
}

/// `tls_mode=require` + the harness CA syncs against the Force-Encryption
/// server — the in-PRELOGIN TDS handshake, chain + hostname verification, and
/// the full read path through TLS.
#[test]
#[ignore = "needs the mssql-tls compose service (integration profile)"]
fn mssql_tls_require_with_ca_reads_rows() {
    let tmp = tempfile::TempDir::new().unwrap();
    let src = mssql_tls(
        tmp.path(),
        "localhost",
        serde_json::json!({"tls_mode": "require", "tls_ca_pem": harness_ca()}),
    );
    let out = Engine::new(&store(tmp.path(), "r"))
        .sync("r", &stream(SyncMode::FullRefresh, None), &src, &arrow())
        .expect("full refresh over TLS");
    assert_eq!(
        out.rows_written, 3,
        "3 seeded contacts through Force-Encryption TLS"
    );
}

/// Encrypt-only opt-out: `tls_insecure_skip_verify` connects without a CA.
#[test]
#[ignore = "needs the mssql-tls compose service (integration profile)"]
fn mssql_tls_skip_verify_reads_rows() {
    let tmp = tempfile::TempDir::new().unwrap();
    let src = mssql_tls(
        tmp.path(),
        "localhost",
        serde_json::json!({"tls_mode": "require", "tls_insecure_skip_verify": true}),
    );
    let out = Engine::new(&store(tmp.path(), "r"))
        .sync("r", &stream(SyncMode::FullRefresh, None), &src, &arrow())
        .expect("full refresh with skip-verify");
    assert_eq!(out.rows_written, 3);
}

/// Negative verification gate: dialing the same server as `127.0.0.1` (the cert
/// is valid ONLY for `localhost`) must fail — verification is real.
#[test]
#[ignore = "needs the mssql-tls compose service (integration profile)"]
fn mssql_tls_wrong_name_fails_verification() {
    let tmp = tempfile::TempDir::new().unwrap();
    let src = mssql_tls(
        tmp.path(),
        "127.0.0.1",
        serde_json::json!({"tls_mode": "require", "tls_ca_pem": harness_ca()}),
    );
    let err = Engine::new(&store(tmp.path(), "w"))
        .sync("w", &stream(SyncMode::FullRefresh, None), &src, &arrow())
        .expect_err("wrong-name dial must fail verification");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("not valid for name")
            || msg.contains("invalid peer certificate")
            || msg.contains("Tls"),
        "failure is a certificate/TLS error: {msg}"
    );
}

/// The engagement proof's other half: a plaintext-mode client cannot talk to a
/// Force-Encryption server.
#[test]
#[ignore = "needs the mssql-tls compose service (integration profile)"]
fn mssql_tls_plaintext_mode_is_rejected() {
    let tmp = tempfile::TempDir::new().unwrap();
    let src = mssql_tls(
        tmp.path(),
        "localhost",
        serde_json::json!({"tls_mode": "disable"}),
    );
    let err = Engine::new(&store(tmp.path(), "w"))
        .sync("w", &stream(SyncMode::FullRefresh, None), &src, &arrow())
        .expect_err("plaintext must be rejected by Force Encryption");
    let msg = format!("{err:?}");
    assert!(!msg.is_empty(), "rejection surfaced: {msg}");
}
