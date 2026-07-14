//! [[WEIR-T-0109]]: the `s3` object-store source as a wasm guest, reading NDJSON from MinIO over
//! HTTP. The host egress signs every request with **SigV4** (`Credential::AwsSigV4`, minioadmin
//! creds) — the secret never enters the guest. `#[ignore]`: needs the integration MinIO
//! (`angreal integration up`). Run with
//! `cargo test -p weir-engine --test wasm_s3_engine -- --ignored --test-threads=1`.

use std::path::Path;

use weir_connector::{Config, ConfiguredStream, MappingSpec, SyncMode, WriteMode};
use weir_engine::{Engine, Store};
use weir_runtime::{ConnectorHandle, Credential, HostAllowList};

fn s3_endpoint() -> String {
    std::env::var("WEIR_TEST_S3_ENDPOINT").unwrap_or_else(|_| "http://localhost:9000".to_string())
}

fn s3(root: &Path) -> ConnectorHandle {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/connectors/s3");
    weir_wasm_testkit::stage(
        &weir_wasm_testkit::WasmPackage {
            fixture_dir: &fixture,
            wasm_file: "weir_s3_wasm.wasm",
            pkg_name: "weir-s3-pkg",
            capabilities: &["http"],
        },
        root,
    );
    let cfg = Config {
        json: format!(
            "{{\"endpoint\":\"{}\",\"bucket\":\"weir-test\"}}",
            s3_endpoint()
        ),
    };
    // Host-side SigV4 signing with MinIO's root creds — the guest never sees the key.
    let egress = HostAllowList::allow_all().with_credential(Credential::AwsSigV4 {
        access_key: "minioadmin".to_string(),
        secret_key: "minioadmin".to_string(),
        region: "us-east-1".to_string(),
        service: "s3".to_string(),
    });
    ConnectorHandle::from_wasm_package(root, "weir-s3-pkg", &cfg, egress, &[])
        .expect("load s3 wasm")
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

fn cstream(sync: SyncMode, cursor: Option<&str>) -> ConfiguredStream {
    ConfiguredStream {
        stream: "objects".to_string(),
        sync_mode: sync,
        cursor_field: cursor.map(str::to_string),
        primary_key: None,
        write_mode: WriteMode::Append,
        mapping: MappingSpec::default(),
    }
}

fn store(tmp: &Path, name: &str) -> Store {
    Store::open(tmp.join(format!("{name}.db")).to_str().unwrap()).expect("store")
}

#[test]
#[ignore = "needs the integration MinIO (angreal integration up)"]
fn s3_wasm_full_refresh_reads_ndjson_objects() {
    let tmp = tempfile::TempDir::new().unwrap();
    let s3 = s3(tmp.path());
    let st = cstream(SyncMode::FullRefresh, None);
    let out = Engine::new(&store(tmp.path(), "r"))
        .sync("r", &st, &s3, &arrow())
        .expect("s3 full-refresh read");
    // events.ndjson (3) + more.ndjson (2) — the host-signed list + gets round-tripped.
    assert_eq!(
        out.rows_written, 5,
        "5 NDJSON records across the two objects"
    );
}

#[test]
#[ignore = "needs the integration MinIO (angreal integration up)"]
fn s3_wasm_incremental_by_key_resumes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let s3 = s3(tmp.path());
    let inc = cstream(SyncMode::Incremental, Some("_key"));
    let rd = store(tmp.path(), "rd");

    // First pass reads both objects (keys sorted: events < more) → 5 rows; cursor = the greatest key.
    let o1 = Engine::new(&rd)
        .sync("rd", &inc, &s3, &arrow())
        .expect("read 1");
    assert_eq!(o1.rows_written, 5);
    assert_eq!(o1.final_state.cursor.as_deref(), Some("more.ndjson"));

    // Second pass: no object key sorts after the cursor → nothing re-read.
    let o2 = Engine::new(&rd)
        .sync("rd", &inc, &s3, &arrow())
        .expect("read 2");
    assert_eq!(o2.rows_written, 0, "no new objects → no re-delivery");
}
